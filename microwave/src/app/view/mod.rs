mod keyboard;

use std::collections::HashMap;

use bevy::camera::ScalingMode;
use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::css;
use bevy::ecs::schedule::common_conditions;
use bevy::prelude::*;
use bevy::ui;
use tune::pitch::Ratio;
use tune::scala::Kbm;
use tune::scala::Scl;

use crate::app::state::BackendState;
use crate::app::state::KeysVersion;
use crate::app::state::LayoutVersion;
use crate::app::state::Menu;
use crate::app::state::OnScreenKeyboards;
use crate::app::state::RenderContext;
use crate::app::state::ViewState;
use crate::app::view::keyboard::KeyboardCreator;
use crate::app::view::keyboard::OnScreenKey;
use crate::app::view::keyboard::OnScreenKeyboard;
use crate::piano::PianoEngineState;
use crate::piano::PressedKeys;
use crate::tuning_layout::TuningLayout;

const LOWER_KEYBOARD_LAYER: RenderLayers = RenderLayers::layer(2);
const UPPER_KEYBOARD_LAYER: RenderLayers = RenderLayers::layer(3);

mod z_index {
    pub const RECORDING_INDICATOR: i32 = 0;
    pub const PITCH_LINES: i32 = 1;
    pub const MENU_BACKDROP: i32 = 2;
    pub const MENU_TEXT: i32 = 3;

    pub mod pitch_lines {
        pub const LINE: i32 = 0;
        pub const CENTS_MARKER: i32 = 1;
        pub const TEXT: i32 = 2;
    }
}

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Srgba::hex("222222").unwrap().into()))
            .add_systems(Startup, (init_scene, init_menu, init_recording_indicators))
            .add_systems(
                Update,
                (
                    update_keyboard_cameras
                        .run_if(common_conditions::resource_changed::<ViewState>),
                    (
                        render_keyboards.run_if(keyboard_layout_changed),
                        update_keyboards.run_if(keyboard_state_changed),
                    )
                        .chain(),
                    render_grid_lines.run_if(keyboard_layout_changed),
                    render_pitch_lines_and_cents_markers.run_if(pressed_keys_changed),
                    render_menu,
                    render_recording_indicators.run_if(recording_indicator_changed),
                ),
            );
    }
}

fn keyboard_layout_changed(layout_version: Res<LayoutVersion>, view_state: Res<ViewState>) -> bool {
    layout_version.is_changed() || view_state.is_changed()
}

fn keyboard_state_changed(
    layout_version: Res<LayoutVersion>,
    keys_version: Res<KeysVersion>,
    view_state: Res<ViewState>,
) -> bool {
    layout_version.is_changed() || keys_version.is_changed() || view_state.is_changed()
}

fn pressed_keys_changed(keys_version: Res<KeysVersion>, view_state: Res<ViewState>) -> bool {
    keys_version.is_changed() || view_state.is_changed()
}

fn recording_indicator_changed(
    view_state: Res<ViewState>,
    backend_state: Res<BackendState>,
) -> bool {
    view_state.is_changed() || backend_state.is_changed()
}

#[derive(Component)]
struct BackgroundCamera;

#[derive(Component, Clone, Copy)]
enum KeyboardViewport {
    Upper,
    Lower,
}

fn init_scene(mut commands: Commands) {
    create_background_2d_camera(&mut commands);
    create_keyboard_3d_camera(&mut commands, UPPER_KEYBOARD_LAYER, KeyboardViewport::Upper);
    create_keyboard_3d_camera(&mut commands, LOWER_KEYBOARD_LAYER, KeyboardViewport::Lower);
    create_2d_camera(&mut commands);
    create_light(&mut commands);
}

fn create_background_2d_camera(commands: &mut Commands) {
    commands.spawn((
        Camera2d,
        BackgroundCamera,
        Camera {
            order: -1,
            ..default()
        },
    ));
}

fn create_keyboard_3d_camera(
    commands: &mut Commands,
    render_layers: RenderLayers,
    viewport: KeyboardViewport,
) {
    commands.spawn((
        Camera3d::default(),
        viewport,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal {
                viewport_width: 1.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        render_layers,
    ));
}

fn create_2d_camera(commands: &mut Commands) {
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}

fn create_light(commands: &mut Commands) {
    commands.spawn((
        PointLight {
            intensity: 25_000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.5, 0.2),
        LOWER_KEYBOARD_LAYER.union(&UPPER_KEYBOARD_LAYER),
    ));
}

fn update_keyboard_cameras(
    mut keyboard_cameras: Query<(&mut Camera, &KeyboardViewport)>,
    view_state: Res<ViewState>,
) {
    let window_size = view_state.resolution.physical_size();
    let third = window_size.y / 3;

    for (mut camera, viewport) in &mut keyboard_cameras {
        let physical_position = match viewport {
            KeyboardViewport::Upper => UVec2::new(0, window_size.y - 2 * third),
            KeyboardViewport::Lower => UVec2::new(0, window_size.y - third),
        };

        camera.viewport = Some(Viewport {
            physical_position,
            physical_size: UVec2::new(window_size.x, third),
            depth: 0.0..1.0,
        });
    }
}

fn render_keyboards(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
) {
    log::trace!("Recreating keyboard",);

    commands.despawn_all::<With<OnScreenKeyboard>>();

    create_keyboards(
        &mut commands,
        &mut meshes,
        &mut materials,
        &engine_state.curr_tuning_layout,
        &view_state,
    );
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tuning_layout: &TuningLayout,
    view_state: &ViewState,
) {
    let (isomorphic, linear, reference) = match view_state.on_screen_keyboard.curr_option() {
        OnScreenKeyboards::None => (None, None, None),
        OnScreenKeyboards::Isomorphic => (Some(LOWER_KEYBOARD_LAYER), None, None),
        OnScreenKeyboards::Linear => (None, Some(LOWER_KEYBOARD_LAYER), None),
        OnScreenKeyboards::Reference => (None, None, Some(LOWER_KEYBOARD_LAYER)),
        OnScreenKeyboards::IsomorphicAndReference => {
            (Some(UPPER_KEYBOARD_LAYER), None, Some(LOWER_KEYBOARD_LAYER))
        }
        OnScreenKeyboards::LinearAndReference => {
            (None, Some(UPPER_KEYBOARD_LAYER), Some(LOWER_KEYBOARD_LAYER))
        }
    };

    let mut creator = KeyboardCreator {
        commands,
        meshes,
        materials,
        view_state,
        depth: view_state.height() / view_state.width() * core::f32::consts::SQRT_2 / 3.0,
    };

    if let Some(layer) = isomorphic {
        creator.create_isomorphic(tuning_layout, 0, &layer);
    }

    if let Some(layer) = linear {
        creator.create_linear(tuning_layout, 0, &layer);
    }

    if let Some(layer) = reference {
        creator.create_linear(
            &view_state.reference_tuning_layout,
            view_state
                .reference_tuning_layout
                .kbm
                .root
                .ref_key
                .num_keys_before(tuning_layout.kbm.root.ref_key),
            &layer,
        );
    }
}

fn update_keyboards(
    keyboards: Query<&OnScreenKeyboard>,
    mut keys: Query<(
        &mut OnScreenKey,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    engine_state: Res<PianoEngineState>,
    mut press_amounts: Local<HashMap<Entity, f32>>,
) {
    for keyboard in &keyboards {
        press_amounts.clear();

        for &(pitch, _velocity) in engine_state.pressed_keys.values() {
            if let Some(pitch) = pitch {
                for (entity, press_amount) in keyboard.keys_for_pitch(pitch) {
                    *press_amounts.entry(entity).or_insert(0.0) += press_amount;
                }
            }
        }

        for entity in keyboard.key_entities() {
            let press_amount = press_amounts.get(&entity).copied().unwrap_or_default();

            let Ok((mut key, mut transform, material)) = keys.get_mut(entity) else {
                continue;
            };

            if key.press_amount == press_amount {
                continue;
            }
            key.press_amount = press_amount;

            *transform = key.base_transform;
            transform.rotate_around(
                key.pivot,
                Quat::from_rotation_x((1.5 * press_amount).to_radians()),
            );

            let mut material_asset = materials.get_mut(&material.0).unwrap();
            let base_color = material_asset.base_color.to_srgba();
            material_asset.emissive =
                (base_color * 0.5 + base_color * press_amount * 1.0 + css::GRAY * press_amount)
                    .into();
        }
    }
}

#[derive(Component)]
struct GridLines;

fn render_grid_lines(
    mut commands: Commands,
    background_camera: Single<Entity, With<BackgroundCamera>>,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
) {
    log::trace!("Recreating grid lines");

    commands.despawn_all::<With<GridLines>>();

    create_grid_lines(
        &mut commands,
        *background_camera,
        &engine_state.curr_tuning_layout.scl,
        &engine_state.curr_tuning_layout.kbm,
        &view_state,
    );
}

fn create_grid_lines(
    commands: &mut Commands,
    camera: Entity,
    scl: &Scl,
    kbm: &Kbm,
    view_state: &ViewState,
) {
    let tuning = (scl, kbm.root);

    commands
        .spawn((
            GridLines,
            Node {
                position_type: PositionType::Absolute,
                width: ui::percent(100),
                height: ui::percent(100),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|parent| {
            for (degree, pitch_coord) in view_state.world_coords_of_tuning(&tuning) {
                let line_color = match degree {
                    0 => css::SALMON,
                    _ => css::GRAY,
                };

                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: ui::percent((pitch_coord + 0.5) * 100.0),
                        width: ui::px(1),
                        height: ui::percent(100),
                        ..default()
                    },
                    BackgroundColor(line_color.into()),
                ));
            }
        });
}

#[derive(Component)]
struct PitchLines;

fn render_pitch_lines_and_cents_markers(
    mut commands: Commands,
    engine_state: Res<PianoEngineState>,
    view_state: Res<ViewState>,
) {
    log::trace!("Recreating pitch lines and cents markers");

    commands.despawn_all::<With<PitchLines>>();

    create_pitch_lines_and_cents_markers(&mut commands, &engine_state.pressed_keys, &view_state);
}

fn create_pitch_lines_and_cents_markers(
    commands: &mut Commands,
    pressed_keys: &PressedKeys,
    view_state: &ViewState,
) {
    let mut pitches = pressed_keys
        .values()
        .filter_map(|(pitch, _velocity)| *pitch)
        .collect::<Vec<_>>();
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pitches.dedup();

    let line_height = view_state.line_height(pitches.len());
    let font_size = view_state.font_size(pitches.len());
    let octave_range = view_state.pitch_range().as_octaves();

    commands
        .spawn((
            PitchLines,
            Node {
                position_type: PositionType::Absolute,
                width: ui::percent(100),
                height: ui::percent(100),
                ..default()
            },
            GlobalZIndex(z_index::PITCH_LINES),
        ))
        .with_children(|parent| {
            let mut curr_slice_window = pitches.as_slice();

            while let Some((second, others)) = curr_slice_window.split_last() {
                let pitch_coord = view_state.world_coord_of_pitch(*second);
                let pitch_position = ui::percent((pitch_coord + 0.5) * 100.0);
                let mut row = 0;

                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: pitch_position,
                        width: ui::px(1),
                        height: ui::percent(100),
                        ..default()
                    },
                    BackgroundColor(css::WHITE.into()),
                    ZIndex(z_index::pitch_lines::LINE),
                ));

                parent.spawn((
                    Text::new(format!("{:.0} Hz", second.as_hz())),
                    TextFont::from_font_size(font_size),
                    TextColor(css::RED.into()),
                    TextLayout::no_wrap(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: pitch_position,
                        top: ui::px(row as f32 * line_height),
                        ..default()
                    },
                    ZIndex(z_index::pitch_lines::TEXT),
                ));
                row += 1;

                for first in others.iter() {
                    let approximation = Ratio::between_pitches(*first, *second)
                        .nearest_fraction(view_state.odd_limit);

                    let width = (approximation.deviation.as_octaves() / octave_range) as f32
                        * view_state.width();

                    let color = match width > 0.0 {
                        true => css::GREEN,
                        false => css::MAROON,
                    };

                    let (bar_left, bar_right) = match width > 0.0 {
                        true => (Val::Auto, ui::percent((0.5 - pitch_coord) * 100.0)),
                        false => (pitch_position, Val::Auto),
                    };

                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: bar_left,
                            right: bar_right,
                            top: ui::px(row as f32 * line_height),
                            width: ui::px(width.abs()),
                            height: ui::px(line_height),
                            ..default()
                        },
                        BackgroundColor(color.into()),
                        ZIndex(z_index::pitch_lines::CENTS_MARKER),
                    ));

                    parent.spawn((
                        Text::new(format!(
                            "{}/{} [{:.0}c]",
                            approximation.numer,
                            approximation.denom,
                            approximation.deviation.as_cents().abs()
                        )),
                        TextFont::from_font_size(font_size),
                        TextColor(Color::WHITE),
                        TextLayout::no_wrap(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: pitch_position,
                            top: ui::px(row as f32 * line_height),
                            ..default()
                        },
                        ZIndex(z_index::pitch_lines::TEXT),
                    ));

                    row += 1;
                }

                curr_slice_window = others;
            }
        });
}

#[derive(Component)]
struct MenuBackdrop;

#[derive(Component)]
struct MenuText;

fn init_menu(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("FiraMono-Regular.ttf");

    commands.spawn((
        MenuBackdrop,
        Node {
            position_type: PositionType::Absolute,
            width: ui::percent(100),
            height: ui::percent(100),
            ..default()
        },
        BackgroundColor(css::BLACK.with_alpha(0.75).into()),
        GlobalZIndex(z_index::MENU_BACKDROP),
    ));

    commands.spawn((
        MenuText,
        Text::new(""),
        TextFont::default().with_font(font),
        TextColor(css::LIME.into()),
        TextLayout::no_wrap(),
        Node {
            position_type: PositionType::Absolute,
            left: ui::px(0),
            top: ui::px(0),
            ..default()
        },
        GlobalZIndex(z_index::MENU_TEXT),
    ));
}

fn render_menu(
    mut backdrops: Query<&mut Visibility, With<MenuBackdrop>>,
    mut menus: Query<(&mut Text, &mut TextFont), With<MenuText>>,
    menu: Res<Menu>,
    engine_state: Res<PianoEngineState>,
    backend_state: Res<BackendState>,
    view_state: Res<ViewState>,
    key_code: Res<ButtonInput<KeyCode>>,
    mut text_buffer: Local<String>,
) {
    let alt_pressed = key_code.pressed(KeyCode::AltLeft) || key_code.pressed(KeyCode::AltRight);

    let backdrop_visibility = match alt_pressed {
        true => Visibility::Visible,
        false => Visibility::Hidden,
    };

    for mut visibility in &mut backdrops {
        visibility.set_if_neq(backdrop_visibility);
    }

    for (mut text, mut text_font) in &mut menus {
        text_buffer.clear();

        let ctx = RenderContext {
            output: &mut text_buffer,
            engine_state: &engine_state,
            backend_state: &backend_state,
            view_state: &view_state,
        };

        match alt_pressed {
            true => menu.render_full(ctx),
            false => menu.render_light(ctx),
        }

        if text.0 != *text_buffer {
            text.0.clone_from(&text_buffer);
        }

        let font_size = view_state.font_size(text_buffer.lines().count());
        if text_font.font_size != font_size {
            text_font.font_size = font_size;
        }
    }
}

#[derive(Component)]
#[require(Node)]
struct RecordingIndicator;

fn init_recording_indicators(mut commands: Commands) {
    commands.spawn((
        RecordingIndicator,
        BackgroundColor(css::RED.into()),
        GlobalZIndex(z_index::RECORDING_INDICATOR),
    ));
}

fn render_recording_indicators(
    mut recording_indicators: Query<(&mut Node, &mut Visibility), With<RecordingIndicator>>,
    view_state: Res<ViewState>,
    aggregate: Res<BackendState>,
) {
    let recording_active = !aggregate.recorder_details.is_empty();
    let visibility = match recording_active {
        true => Visibility::Visible,
        false => Visibility::Hidden,
    };
    let indicator_size = view_state.line_height(6) * 4.0;

    let node = Node {
        position_type: PositionType::Absolute,
        top: ui::px(indicator_size * 0.25),
        right: ui::px(indicator_size * 0.25),
        width: ui::px(indicator_size),
        height: ui::px(indicator_size),
        border_radius: BorderRadius::MAX,
        ..default()
    };

    for (mut curr_node, mut curr_visibility) in &mut recording_indicators {
        curr_node.set_if_neq(node.clone());
        curr_visibility.set_if_neq(visibility);
    }
}
