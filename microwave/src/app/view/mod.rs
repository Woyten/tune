mod keyboard;

use bevy::camera::ScalingMode;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Anchor;
use tune::pitch::Ratio;
use tune::scala::Kbm;
use tune::scala::Scl;
use tune::tuning::Scale;

use crate::app::state::BackendState;
use crate::app::state::Menu;
use crate::app::state::OnScreenKeyboards;
use crate::app::state::RenderContext;
use crate::app::state::ViewState;
use crate::app::view::keyboard::KeyboardCreator;
use crate::app::view::keyboard::OnScreenKeyboard;
use crate::piano::PianoEngineState;
use crate::piano::PressedKeys;
use crate::tunable;
use crate::tuning_layout::TuningLayout;

pub const BACKGROUND_LAYER: RenderLayers = RenderLayers::layer(1);

mod z_index {
    pub const RECORDING_INDICATOR: f32 = 0.0;
    pub const MENU_TEXT_LIGHT: f32 = 0.1;
    pub const PITCH_LINE: f32 = 0.2;
    pub const PITCH_TEXT: f32 = 0.3;
    pub const CENTS_MARKER: f32 = 0.4;
    pub const CENTS_TEXT: f32 = 0.5;
    pub const MENU_BACKDROP: f32 = 0.6;
    pub const MENU_TEXT_FULL: f32 = 0.7;
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Srgba::hex("222222").unwrap().into()))
            .add_systems(Startup, (init_scene, init_menu, init_recording_indicators))
            .add_systems(
                Update,
                (
                    (render_keyboards, update_keyboards).chain(),
                    render_grid_lines,
                    render_pitch_lines_and_cents_markers,
                    render_menu,
                    render_recording_indicators,
                ),
            );
    }
}

fn init_scene(mut commands: Commands) {
    create_background_2d_camera(&mut commands);
    create_3d_camera(&mut commands);
    create_2d_camera(&mut commands);
    create_light(&mut commands, Transform::from_xyz(-0.25, 7.5, -7.5));
    create_light(&mut commands, Transform::from_xyz(0.25, 7.5, -7.5));
}

fn create_background_2d_camera(commands: &mut Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            ..default()
        },
        BACKGROUND_LAYER,
    ));
}

fn create_3d_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal {
                viewport_width: 1.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::NEG_Z),
    ));
}

fn create_2d_camera(commands: &mut Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
    ));
}

fn create_light(commands: &mut Commands, transform: Transform) {
    commands.spawn((
        PointLight {
            intensity: 10000.0,
            ..default()
        },
        transform,
    ));
}

fn render_keyboards(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
    mut last_layout_version: Local<u64>,
) {
    if is_changed(&mut *last_layout_version, engine_state.layout_version) || view_state.is_changed()
    {
        log::trace!("Recreating keyboard",);

        // Remove old keyboards
        for (entity, _) in &keyboards {
            commands.entity(entity).despawn();
        }

        create_keyboards(
            &mut commands,
            &mut meshes,
            &mut materials,
            &engine_state.curr_tuning_layout,
            &view_state,
        );
    }
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tuning_layout: &TuningLayout,
    view_state: &ViewState,
) {
    const KEYBOARD_VERT_FILL: f32 = 0.85;

    let (reference_keyboard_location, scale_keyboard_location, keyboard_location) =
        match view_state.on_screen_keyboard.curr_option() {
            OnScreenKeyboards::Isomorphic => (None, None, Some(1.0 / 3.0)),
            OnScreenKeyboards::Scale => (None, Some(1.0 / 3.0), None),
            OnScreenKeyboards::Reference => (Some(1.0 / 3.0), None, None),
            OnScreenKeyboards::IsomorphicAndReference => (Some(1.0 / 3.0), None, Some(0.0)),
            OnScreenKeyboards::ScaleAndReference => (Some(1.0 / 3.0), Some(0.0), None),
            OnScreenKeyboards::None => (None, None, None),
        };

    let mut creator = KeyboardCreator {
        commands,
        meshes,
        materials,
        view_state,
        height: view_state.height_3d / 3.0 * KEYBOARD_VERT_FILL,
        width: 1.0,
    };

    if let Some(reference_keyboard_location) = reference_keyboard_location {
        creator.create_linear(
            &view_state.reference_tuning_layout,
            view_state
                .reference_tuning_layout
                .kbm
                .root
                .ref_key
                .num_keys_before(tuning_layout.kbm.root.ref_key),
            view_state.height_3d * reference_keyboard_location,
        );
    }

    if let Some(scale_keyboard_location) = scale_keyboard_location {
        creator.create_linear(
            tuning_layout,
            0,
            view_state.height_3d * scale_keyboard_location,
        );
    }

    if let Some(keyboard_location) = keyboard_location {
        creator.create_isomorphic(tuning_layout, 0, view_state.height_3d * keyboard_location);
    }
}

fn update_keyboards(
    keyboards: Query<&mut OnScreenKeyboard>,
    mut keys: Query<&mut Transform>,
    engine_state: Res<PianoEngineState>,
) {
    for keyboard in keyboards {
        for key in keyboard.get_all_keys() {
            *keys.get_mut(key.transform).unwrap() = key.orig_transform;
        }

        for &(pitch, _velocity) in engine_state.pressed_keys.values() {
            if let Some(pitch) = pitch {
                for (key, amount) in keyboard.get_keys_for_pitch(pitch) {
                    let mut transform = keys.get_mut(key.transform).unwrap();
                    transform.rotate_around(
                        key.rotation_point,
                        Quat::from_rotation_x((1.5 * amount as f32).to_radians()),
                    );
                }
            }
        }
    }
}

#[derive(Component)]
struct GridLines;

fn render_grid_lines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    grid_lines: Query<Entity, With<GridLines>>,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
    mut last_layout_version: Local<u64>,
) {
    if is_changed(&mut *last_layout_version, engine_state.layout_version) || view_state.is_changed()
    {
        log::trace!("Recreating grid lines");

        // Remove old grid lines
        for entity in &grid_lines {
            commands.entity(entity).despawn();
        }

        create_grid_lines(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            &engine_state.curr_tuning_layout.scl,
            &engine_state.curr_tuning_layout.kbm,
            &view_state,
        );
    }
}

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    scl: &Scl,
    kbm: &Kbm,
    view_state: &ViewState,
) {
    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip, default());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(0.0, view_state.bottom(), 0.0),
                Vec3::new(0.0, view_state.top(), 0.0),
            ],
        );
        mesh
    });

    let mut scale_grid = commands.spawn((
        GridLines,
        Transform::default(),
        Visibility::default(),
        BACKGROUND_LAYER,
    ));

    let tuning = (scl, kbm.root);
    for (degree, pitch_coord) in view_state.world_coords_of_tuning(&tuning) {
        let line_color = match degree {
            0 => css::SALMON,
            _ => css::GRAY,
        };

        scale_grid.with_children(|commands| {
            commands.spawn((
                Mesh2d(line_mesh.clone()),
                MeshMaterial2d(color_materials.add(ColorMaterial::from_color(line_color))),
                Transform::from_xyz(pitch_coord * view_state.width_2d, 0.0, 0.0),
                BACKGROUND_LAYER,
            ));
        });
    }
}

#[derive(Component)]
struct PitchLines;

fn render_pitch_lines_and_cents_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    pitch_lines: Query<Entity, With<PitchLines>>,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
    mut last_keys_version: Local<u64>,
) {
    if is_changed(&mut *last_keys_version, engine_state.keys_version) || view_state.is_changed() {
        log::trace!("Recreating pitch lines and cents markers");

        // Remove old pitch lines
        for entity in &pitch_lines {
            commands.entity(entity).despawn();
        }

        create_pitch_lines_and_cents_markers(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            &engine_state.pressed_keys,
            &view_state,
        );
    }
}

fn create_pitch_lines_and_cents_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    pressed_keys: &PressedKeys,
    view_state: &ViewState,
) {
    let line_height = view_state.line_height(pressed_keys.len());
    let font_size = view_state.font_size(pressed_keys.len());
    let first_line_center = view_state.top() - line_height / 2.0;

    let mut scale_grid_canvas =
        commands.spawn((PitchLines, Transform::default(), Visibility::default()));

    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip, default());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(0.0, view_state.bottom(), 0.0),
                Vec3::new(0.0, view_state.top(), 0.0),
            ],
        );
        mesh
    });

    let square_mesh = meshes.add(Rectangle::default());

    let octave_range = view_state.pitch_range().as_octaves();

    let mut pitches = pressed_keys
        .values()
        .filter_map(|(pitch, _velocity)| *pitch)
        .collect::<Vec<_>>();
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = pitches.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = view_state.world_coord_of_pitch(*second) * view_state.width_2d;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn((
                Mesh2d(line_mesh.clone()),
                MeshMaterial2d(color_materials.add(Color::WHITE)),
                Transform::from_xyz(pitch_coord, 0.0, z_index::PITCH_LINE),
            ));
        });

        let mut curr_line_center = first_line_center;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn((
                Text2d::from(format!("{:.0} Hz", second.as_hz())),
                TextFont::from_font_size(font_size),
                TextColor(css::RED.into()),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(pitch_coord, curr_line_center, z_index::PITCH_TEXT),
            ));
        });

        curr_line_center -= line_height;

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(view_state.odd_limit);

            let width =
                (approximation.deviation.as_octaves() / octave_range) as f32 * view_state.width_2d;

            let color = if width > 0.0 { css::GREEN } else { css::MAROON };

            scale_grid_canvas.with_children(|commands| {
                commands.spawn((
                    Mesh2d(square_mesh.clone()),
                    MeshMaterial2d(color_materials.add(ColorMaterial::from_color(color))),
                    Transform::from_xyz(
                        pitch_coord - width / 2.0,
                        curr_line_center,
                        z_index::CENTS_MARKER,
                    )
                    .with_scale(Vec3::new(width.abs(), line_height, 0.0)),
                ));
                commands.spawn((
                    Text2d::new(format!(
                        "{}/{} [{:.0}c]",
                        approximation.numer,
                        approximation.denom,
                        approximation.deviation.as_cents().abs()
                    )),
                    TextFont::from_font_size(font_size),
                    TextColor(Color::WHITE),
                    Anchor::CENTER_LEFT,
                    Transform::from_xyz(pitch_coord, curr_line_center, z_index::CENTS_TEXT)
                        .with_scale(compress_text()),
                ));
            });

            curr_line_center -= line_height;
        }

        curr_slice_window = others;
    }
}

fn iterate_grid_world_coords<'a>(
    view_state: &'a ViewState,
    tuning: &'a impl Scale,
) -> impl Iterator<Item = (i32, f32)> + 'a {
    tunable::range(tuning, view_state.viewport_left, view_state.viewport_right).map(
        move |key_degree| {
            (
                key_degree,
                view_state.world_coord_of_pitch(tuning.sorted_pitch_of(key_degree)),
            )
        },
    )
}

#[derive(Component)]
struct MenuBackdrop;

#[derive(Component)]
struct MenuText;

fn init_menu(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
    let font = assets.load("FiraMono-Regular.ttf");

    commands.spawn((
        MenuBackdrop,
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(color_materials.add(ColorMaterial::from_color(css::BLACK.with_alpha(0.9)))),
        Transform::from_xyz(0.0, 0.0, z_index::MENU_BACKDROP),
    ));

    commands.spawn((
        MenuText,
        Text2d::default(),
        TextFont::default().with_font(font),
        TextColor(css::LIME.into()),
        Anchor::TOP_LEFT,
        Transform::from_xyz(0.0, 0.0, z_index::MENU_TEXT_FULL),
    ));
}

#[expect(clippy::type_complexity)]
fn render_menu(
    mut backdrops: Query<
        (&mut Transform, &mut Visibility),
        (With<MenuBackdrop>, Without<MenuText>),
    >,
    mut menus: Query<
        (&mut Transform, &mut Text2d, &mut TextFont),
        (With<MenuText>, Without<MenuBackdrop>),
    >,
    menu: Res<Menu>,
    engine_state: Res<PianoEngineState>,
    backend_state: Res<BackendState>,
    view_state: Res<ViewState>,
    key_code: Res<ButtonInput<KeyCode>>,
) {
    let alt_pressed = key_code.pressed(KeyCode::AltLeft) || key_code.pressed(KeyCode::AltRight);

    for (mut transform, mut visibility) in &mut backdrops {
        transform.scale = Vec3::new(view_state.width_2d, view_state.height_2d, 0.0);
        *visibility = match alt_pressed {
            true => Visibility::Visible,
            false => Visibility::Hidden,
        };
    }

    for (mut transform, mut text, mut text_font) in &mut menus {
        transform.translation.x = view_state.left() + 1e-3; // Hack required to make text rendering work on startup
        transform.translation.y = view_state.top() - 1e-3; // Hack required to make text rendering work on startup
        transform.scale = compress_text();

        text.clear();

        let ctx = RenderContext {
            output: &mut text,
            engine_state: &engine_state,
            backend_state: &backend_state,
            view_state: &view_state,
        };

        match alt_pressed {
            true => {
                menu.render_full(ctx);
                transform.translation.z = z_index::MENU_TEXT_FULL;
            }
            false => {
                menu.render_light(ctx);
                transform.translation.z = z_index::MENU_TEXT_LIGHT;
            }
        }

        text_font.font_size = view_state.font_size(text.0.lines().count())
    }
}

#[derive(Component)]
struct RecordingIndicator;

fn init_recording_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        RecordingIndicator,
        Mesh2d(meshes.add(Circle::default())),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(css::RED))),
        Transform::from_xyz(0.0, 0.0, z_index::RECORDING_INDICATOR),
    ));
}

fn render_recording_indicators(
    mut recording_indicators: Query<(&mut Transform, &mut Visibility), With<RecordingIndicator>>,
    view_state: Res<ViewState>,
    aggregate: Res<BackendState>,
) {
    let recording_active = !aggregate.recorder_details.is_empty();
    let indicator_size = view_state.line_height(6) * 4.0;

    for (mut transform, mut visibility) in &mut recording_indicators {
        transform.translation.x = view_state.right() - indicator_size * 0.75;
        transform.translation.y = view_state.top() - indicator_size * 0.75;
        transform.scale = Vec3::splat(indicator_size);
        *visibility = match recording_active {
            true => Visibility::Visible,
            false => Visibility::Hidden,
        };
    }
}

fn compress_text() -> Vec3 {
    Vec3::ONE.with_x(0.9)
}

fn is_changed<T: PartialEq>(last: &mut T, curr: T) -> bool {
    let is_changed = &curr != last;
    *last = curr;
    is_changed
}
