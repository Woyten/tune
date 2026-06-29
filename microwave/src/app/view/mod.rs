use std::collections::BTreeMap;
use std::f32::consts;
use std::fmt;
use std::fmt::Display;

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
use tune_cli::shared::midi::TuningMethod;

use crate::app::PipelineEvent;
use crate::app::resources::MenuResource;
use crate::app::resources::PipelineEventsResource;
use crate::app::resources::ViewSettings;
use crate::app::view::on_screen_keyboard::KeyboardCreator;
use crate::app::view::on_screen_keyboard::OnScreenKeyboard;
use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::piano::PressedKeys;
use crate::pipeline::NoAudioEvent;
use crate::tunable;
use crate::tuning_layout::OnScreenKeyboards;
use crate::tuning_layout::TuningLayout;

mod on_screen_keyboard;

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
            .insert_resource(PipelineAggregate::default())
            .add_systems(Startup, (init_scene, init_menu, init_recording_indicators))
            .add_systems(
                Update,
                (
                    (
                        handle_engine_state,
                        (
                            (render_keyboard, update_keyboard).chain(),
                            render_grid_lines,
                            render_pitch_lines_and_cents_marker,
                        ),
                    )
                        .chain(),
                    (
                        handle_pipeline_events,
                        (render_menu, render_recording_indicators),
                    )
                        .chain(),
                ),
            );
    }
}

#[derive(Default, Resource)]
pub struct PipelineAggregate {
    pub backend: String,
    pub program: Option<String>,
    pub bank: Option<String>,
    pub envelope: Option<String>,
    pub recorder_details: BTreeMap<usize, String>,
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

fn handle_engine_state(
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    mut keys: Query<&mut Transform>,
    engine: Res<PianoEngine>,
    mut state: ResMut<PianoEngineState>,
) {
    // Bring keys to neutral position
    press_or_lift_keys(&keyboards, &mut keys, &state.pressed_keys, -1.0);

    engine.capture_state_into(&mut state);
}

fn render_keyboard(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    state: Res<PianoEngineState>,
    view_settings: Res<ViewSettings>,
    mut last_layout_version: Local<u64>,
) {
    if is_changed(&mut *last_layout_version, state.layout_version) || view_settings.is_changed() {
        log::trace!("Recreating keyboard",);

        // Remove old keyboards
        for (entity, _) in &keyboards {
            commands.entity(entity).despawn();
        }

        create_keyboards(
            &mut commands,
            &mut meshes,
            &mut materials,
            &state.curr_tuning_layout,
            &view_settings,
        );
    }
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tuning_layout: &TuningLayout,
    view_settings: &ViewSettings,
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
        match view_settings.on_screen_keyboard.curr_option() {
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
        view_settings,
        height: SCENE_HEIGHT_3D / 3.0 * KEYBOARD_VERT_FILL,
        width: 1.0,
    };

    if let Some(reference_keyboard_location) = reference_keyboard_location {
        creator.create_linear(
            (
                view_settings.reference_scl.clone(),
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

fn update_keyboard(
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    mut keys: Query<&mut Transform>,
    state: Res<PianoEngineState>,
) {
    press_or_lift_keys(&keyboards, &mut keys, &state.pressed_keys, 1.0);
}

fn press_or_lift_keys(
    keyboards: &Query<(Entity, &mut OnScreenKeyboard)>,
    keys: &mut Query<&mut Transform>,
    pressed_keys: &PressedKeys,
    direction: f32,
) {
    for (_, keyboard) in keyboards {
        for &pitch in pressed_keys.values().flatten() {
            for (key, amount) in keyboard.get_keys(pitch) {
                let mut transform = keys.get_mut(key.entity).unwrap();
                transform.rotate_around(
                    key.rotation_point,
                    Quat::from_rotation_x((1.5 * direction * amount as f32).to_radians()),
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
    state: Res<PianoEngineState>,
    view_settings: Res<ViewSettings>,
    mut last_layout_version: Local<u64>,
) {
    if is_changed(&mut *last_layout_version, state.layout_version) || view_settings.is_changed() {
        log::trace!("Recreating grid lines");

        // Remove old grid lines
        for entity in &grid_lines {
            commands.entity(entity).despawn();
        }

        create_grid_lines(
            &mut commands,
            &mut meshes,
            &mut materials,
            &state.curr_tuning_layout.scl,
            &state.curr_tuning_layout.kbm,
            &view_settings,
        );
    }
}

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    scl: &Scl,
    kbm: &Kbm,
    view_settings: &ViewSettings,
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
    for (degree, pitch_coord) in iterate_grid_coords(view_settings, &tuning) {
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

fn render_pitch_lines_and_cents_marker(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    pitch_lines: Query<Entity, With<PitchLines>>,
    state: Res<PianoEngineState>,
    view_settings: Res<ViewSettings>,
    mut last_keys_version: Local<u64>,
) {
    if is_changed(&mut *last_keys_version, state.keys_version) || view_settings.is_changed() {
        log::trace!("Recreating pitch lines and cents markers");

        // Remove old pitch lines
        for entity in &pitch_lines {
            commands.entity(entity).despawn();
        }

        create_pitch_lines_and_cents_markers(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            &state.pressed_keys,
            &view_settings,
        );
    }
}

fn create_pitch_lines_and_cents_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    pressed_keys: &PressedKeys,
    view_settings: &ViewSettings,
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

    let octave_range = view_settings.pitch_range().as_octaves();

    let mut pitches = pressed_keys.values().flatten().copied().collect::<Vec<_>>();
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = pitches.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = view_settings.hor_world_coord(*second) as f32;

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
                Ratio::between_pitches(*first, *second).nearest_fraction(view_settings.odd_limit);

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
    view_settings: &'a ViewSettings,
    tuning: &'a impl Scale,
) -> impl Iterator<Item = (i32, f32)> + 'a {
    tunable::range(
        tuning,
        view_settings.viewport_left,
        view_settings.viewport_right,
    )
    .map(move |key_degree| {
        (
            key_degree,
            view_settings.hor_world_coord(tuning.sorted_pitch_of(key_degree)) as f32,
        )
    })
}

fn handle_pipeline_events(
    events: Res<PipelineEventsResource>,
    mut aggregate: ResMut<PipelineAggregate>,
) {
    for event in events.0.try_iter() {
        match event {
            PipelineEvent::WaveRecorder(event) => match event.file_name {
                Some(file_name) => {
                    aggregate.recorder_details.insert(
                        event.index,
                        format!(
                            "Recording buffers {} and {} into {}",
                            event.in_buffers.0, event.in_buffers.1, file_name
                        ),
                    );
                }
                None => {
                    aggregate.recorder_details.remove(&event.index);
                }
            },
            PipelineEvent::Magnetron(event) => {
                aggregate.backend = "Magnetron".to_owned();
                aggregate.program = Some(format!(
                    "{} - {}",
                    event.waveform_number, event.waveform_name
                ));
                aggregate.bank = None;
                aggregate.envelope = Some(format!(
                    "{}{}",
                    event.envelope_name,
                    if event.is_default_envelope {
                        " (default)"
                    } else {
                        ""
                    }
                ));
            }
            PipelineEvent::Fluid(event) => {
                aggregate.backend = format!(
                    "Fluid | {} | {}",
                    event.soundfont_location,
                    match event.is_tuned {
                        true => "Single Note Tuning Change",
                        false => "Warning: Tuning channels exceeded! Change tuning mode.",
                    },
                );
                aggregate.program = event
                    .program
                    .as_ref()
                    .map(|(number, name)| format!("{number} - {name}"));
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::FluidError(error) => {
                aggregate.backend = format!(
                    "Fluid | {} | Error: {}",
                    error.soundfont_location, error.error_message
                );
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::MidiOut(event) => {
                aggregate.backend = format!(
                    "MIDI Out | {} | {}",
                    event.device,
                    match event.tuning_method {
                        Some(TuningMethod::FullKeyboard) => "Single Note Tuning Change",
                        Some(TuningMethod::FullKeyboardRt) =>
                            "Single Note Tuning Change (real-time)",
                        Some(TuningMethod::Octave1) => "Scale/Octave Tuning (1-Byte)",
                        Some(TuningMethod::Octave1Rt) => "Scale/Octave Tuning (1-Byte) (real-time)",
                        Some(TuningMethod::Octave2) => "Scale/Octave Tuning (2-Byte)",
                        Some(TuningMethod::Octave2Rt) => "Scale/Octave Tuning (2-Byte) (real-time)",
                        Some(TuningMethod::ChannelFineTuning) => "Channel Fine Tuning",
                        Some(TuningMethod::PitchBend) => "Pitch Bend",
                        None => "Warning: Tuning channels exceeded! Change tuning mode.",
                    },
                );
                aggregate.program = Some(format!("{}", event.program_number));
                aggregate.bank = Some(format!(
                    "{}/{}",
                    fmt_option(&event.bank_msb),
                    fmt_option(&event.bank_lsb)
                ));
                aggregate.envelope = None;
            }
            PipelineEvent::MidiOutError(error) => {
                aggregate.backend = format!("MIDI Out | Error: {}", error.error_message);
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::NoAudio(NoAudioEvent) => {
                aggregate.backend = "No Audio".to_owned();
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
        }
    }
}

#[derive(Component)]
struct MenuBackdrop;

#[derive(Component)]
struct Menu;

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
        Menu,
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
    mut menus: Query<(&mut Text2d, &mut Transform), With<Menu>>,
    menu: Res<MenuResource>,
    engine_state: Res<PianoEngineState>,
    backend_state: Res<PipelineAggregate>,
    view_settings: Res<ViewSettings>,
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
                menu.render_full(&mut text, &engine_state, &backend_state, &view_settings);
                transform.translation.z = z_index::MENU_TEXT_FULL;
            }
            false => {
                menu.render_light(&mut text, &engine_state, &backend_state, &view_settings);
                transform.translation.z = z_index::MENU_TEXT_LIGHT;
            }
        }
    }
}

fn fmt_option<T: Display>(opt: &Option<T>) -> impl Display {
    fmt::from_fn(move |f| match opt {
        Some(v) => write!(f, "{}", v),
        None => write!(f, "-"),
    })
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
    aggregate: Res<PipelineAggregate>,
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
