mod keyboard;

use std::f32::consts;

use bevy::camera::ScalingMode;
use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Anchor;
use tune::math;
use tune::note::Note;
use tune::pitch::Ratio;
use tune::scala::Kbm;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::tuning::Scale;

use crate::app::state::BackendState;
use crate::app::state::Menu;
use crate::app::state::ViewState;
use crate::app::view::keyboard::KeyboardCreator;
use crate::app::view::keyboard::OnScreenKeyboard;
use crate::piano::PianoEngineState;
use crate::piano::PressedKeys;
use crate::tunable;
use crate::tuning_layout::OnScreenKeyboards;
use crate::tuning_layout::TuningLayout;

const SCENE_HEIGHT_2D: f32 = 1.0 / 2.0; // Designed for 2:1 viewport ratio
const SCENE_BOTTOM_2D: f32 = -SCENE_HEIGHT_2D / 2.0;
const SCENE_TOP_2D: f32 = SCENE_HEIGHT_2D / 2.0;
const SCENE_HEIGHT_3D: f32 = SCENE_HEIGHT_2D * consts::SQRT_2; // 45-degree ortho perspective
const SCENE_BOTTOM_3D: f32 = -SCENE_HEIGHT_3D / 2.0;
const SCENE_TOP_3D: f32 = SCENE_HEIGHT_3D / 2.0;
const SCENE_LEFT: f32 = -0.5;
const LINE_TO_CHARACTER_RATIO: f32 = 1.2;
const KEYBOARD_VERT_FILL: f32 = 0.85;

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

const FONT_RESOLUTION: f32 = 30.0;

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
    create_3d_camera(&mut commands);
    create_2d_camera(&mut commands);
    create_light(&mut commands, Transform::from_xyz(-0.25, 7.5, -7.5));
    create_light(&mut commands, Transform::from_xyz(0.25, 7.5, -7.5));
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
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal {
                viewport_width: 1.0,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1.0),
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
    fn get_12edo_key_color(key: i32) -> Srgba {
        if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
            css::WHITE * 0.2
        } else {
            css::WHITE
        }
    }

    let kbm_root = tuning_layout.kbm.kbm_root();

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
        height: SCENE_HEIGHT_3D / 3.0 * KEYBOARD_VERT_FILL,
        width: 1.0,
    };

    if let Some(reference_keyboard_location) = reference_keyboard_location {
        creator.create_linear(
            (
                view_state.reference_scl.clone(),
                KbmRoot::from(Note::from_piano_key(kbm_root.ref_key)),
            ),
            |key| get_12edo_key_color(key + kbm_root.ref_key.midi_number()),
            reference_keyboard_location * SCENE_HEIGHT_3D,
        );
    }

    let colors = &tuning_layout.colors();
    let get_key_color =
        |key| colors[usize::from(math::i32_rem_u(key, u16::try_from(colors.len()).unwrap()))];

    if let Some(scale_keyboard_location) = scale_keyboard_location {
        creator.create_linear(
            (tuning_layout.scl.clone(), kbm_root),
            get_key_color,
            scale_keyboard_location * SCENE_HEIGHT_3D,
        );
    }

    if let Some(keyboard_location) = keyboard_location {
        creator.create_isomorphic(
            tuning_layout,
            (tuning_layout.scl.clone(), kbm_root),
            get_key_color,
            keyboard_location * SCENE_HEIGHT_3D,
        );
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

        for &pitch in engine_state.pressed_keys.values().flatten() {
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

#[derive(Component)]
struct GridLines;

fn render_grid_lines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
            &mut materials,
            &engine_state.curr_tuning_layout.scl,
            &engine_state.curr_tuning_layout.kbm,
            &view_state,
        );
    }
}

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    scl: &Scl,
    kbm: &Kbm,
    view_state: &ViewState,
) {
    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip, default());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(0.0, SCENE_BOTTOM_3D, 0.0),
                Vec3::new(0.0, SCENE_TOP_3D, 0.0),
            ],
        );
        mesh
    });

    let mut scale_grid = commands.spawn((GridLines, Transform::default(), Visibility::default()));

    let tuning = (scl, kbm.kbm_root());
    for (degree, pitch_coord) in iterate_grid_coords(view_state, &tuning) {
        let line_color = match degree {
            0 => css::SALMON,
            _ => css::GRAY,
        };

        scale_grid.with_children(|commands| {
            commands.spawn((
                Mesh3d(line_mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: line_color.into(),
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(pitch_coord, -10.0, -10.0),
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
    const LINE_HEIGHT: f32 = calc_font_height(30);
    const FIRST_LINE_CENTER: f32 = SCENE_TOP_2D - LINE_HEIGHT / 2.0;

    let mut scale_grid_canvas =
        commands.spawn((PitchLines, Transform::default(), Visibility::default()));

    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip, default());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(0.0, SCENE_BOTTOM_2D, 0.0),
                Vec3::new(0.0, SCENE_TOP_2D, 0.0),
            ],
        );
        mesh
    });

    let square_mesh = meshes.add(Rectangle::default());

    let octave_range = view_state.pitch_range().as_octaves();

    let mut pitches = pressed_keys.values().flatten().copied().collect::<Vec<_>>();
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = pitches.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = view_state.hor_world_coord(*second) as f32;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn((
                Mesh2d(line_mesh.clone()),
                MeshMaterial2d(color_materials.add(Color::WHITE)),
                Transform::from_xyz(pitch_coord, 0.0, z_index::PITCH_LINE),
            ));
        });

        let mut curr_line_center = FIRST_LINE_CENTER;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn((
                Text2d::from(format!("{:.0} Hz", second.as_hz())),
                TextFont::from_font_size(FONT_RESOLUTION),
                TextColor(css::RED.into()),
                Anchor::CENTER_LEFT,
                Transform::from_xyz(pitch_coord, curr_line_center, z_index::PITCH_TEXT).with_scale(
                    Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION / LINE_TO_CHARACTER_RATIO),
                ),
            ));
        });

        curr_line_center -= LINE_HEIGHT;

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(view_state.odd_limit);

            let width = (approximation.deviation.as_octaves() / octave_range) as f32;

            let color = if width > 0.0 { css::GREEN } else { css::MAROON };

            scale_grid_canvas.with_children(|commands| {
                let mut transform = Transform::from_xyz(
                    pitch_coord - width / 2.0,
                    curr_line_center,
                    z_index::CENTS_MARKER,
                );
                commands.spawn((
                    Mesh2d(square_mesh.clone()),
                    MeshMaterial2d(color_materials.add(ColorMaterial::from_color(color))),
                    transform.with_scale(Vec3::new(width.abs(), LINE_HEIGHT, 0.0)),
                ));
                transform.translation.z = z_index::CENTS_TEXT;
                commands.spawn((
                    Text2d::new(format!(
                        "{}/{} [{:.0}c]",
                        approximation.numer,
                        approximation.denom,
                        approximation.deviation.as_cents().abs()
                    )),
                    TextFont::from_font_size(FONT_RESOLUTION),
                    TextColor(Color::WHITE),
                    Anchor::CENTER_LEFT,
                    compress_text(transform.with_scale(Vec3::splat(
                        LINE_HEIGHT / FONT_RESOLUTION / LINE_TO_CHARACTER_RATIO,
                    ))),
                ));
            });

            curr_line_center -= LINE_HEIGHT;
        }

        curr_slice_window = others;
    }
}

fn iterate_grid_coords<'a>(
    view_state: &'a ViewState,
    tuning: &'a impl Scale,
) -> impl Iterator<Item = (i32, f32)> + 'a {
    tunable::range(tuning, view_state.viewport_left, view_state.viewport_right).map(
        move |key_degree| {
            (
                key_degree,
                view_state.hor_world_coord(tuning.sorted_pitch_of(key_degree)) as f32,
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
    const LINE_HEIGHT: f32 = calc_font_height(45);

    let font = assets.load("FiraMono-Regular.ttf");

    commands.spawn((
        MenuBackdrop,
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(color_materials.add(ColorMaterial::from_color(css::BLACK.with_alpha(0.9)))),
        Transform::from_xyz(0.0, 0.0, z_index::MENU_BACKDROP).with_scale(Vec3::new(
            1.0,
            SCENE_HEIGHT_2D,
            0.0,
        )),
    ));

    commands.spawn((
        MenuText,
        Text2d::default(),
        TextFont::from_font_size(FONT_RESOLUTION).with_font(font),
        TextColor(css::LIME.into()),
        Anchor::TOP_LEFT,
        compress_text(
            Transform::from_xyz(
                SCENE_LEFT + 1e-6, // Hack required to make text rendering work on startup
                SCENE_TOP_2D,
                z_index::MENU_TEXT_FULL,
            )
            .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION)),
        ),
    ));
}

fn render_menu(
    mut backdrops: Query<&mut Visibility, With<MenuBackdrop>>,
    mut menus: Query<(&mut Text2d, &mut Transform), With<MenuText>>,
    menu: Res<Menu>,
    engine_state: Res<PianoEngineState>,
    backend_state: Res<BackendState>,
    view_state: Res<ViewState>,
    key_code: Res<ButtonInput<KeyCode>>,
) {
    let alt_pressed = key_code.pressed(KeyCode::AltLeft) || key_code.pressed(KeyCode::AltRight);

    for mut visibility in &mut backdrops {
        *visibility = match alt_pressed {
            true => Visibility::Visible,
            false => Visibility::Hidden,
        };
    }

    for (mut text, mut transform) in &mut menus {
        text.clear();
        match alt_pressed {
            true => {
                menu.render_full(&mut text, &engine_state, &backend_state, &view_state);
                transform.translation.z = z_index::MENU_TEXT_FULL;
            }
            false => {
                menu.render_light(&mut text, &engine_state, &backend_state, &view_state);
                transform.translation.z = z_index::MENU_TEXT_LIGHT;
            }
        }
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
        Transform::from_xyz(0.5 - 0.05, 0.25 - 0.05, z_index::RECORDING_INDICATOR)
            .with_scale(Vec3::splat(0.05)),
    ));
}

fn render_recording_indicators(
    mut recording_indicator_visibilities: Query<&mut Visibility, With<RecordingIndicator>>,
    aggregate: Res<BackendState>,
) {
    let recording_active = !aggregate.recorder_details.is_empty();
    for mut visibility in &mut recording_indicator_visibilities {
        *visibility = if recording_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

const fn calc_font_height(num_lines_on_screen: u16) -> f32 {
    SCENE_HEIGHT_2D / num_lines_on_screen as f32 / LINE_TO_CHARACTER_RATIO
}

fn compress_text(mut transform: Transform) -> Transform {
    transform.scale.x *= 0.9;
    transform
}

fn is_changed<T: PartialEq>(last: &mut T, curr: T) -> bool {
    let is_changed = &curr != last;
    *last = curr;
    is_changed
}
