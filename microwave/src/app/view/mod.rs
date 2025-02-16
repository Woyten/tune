use std::collections::BTreeMap;
use std::f32::consts;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Anchor;
use tune::math;
use tune::note::Note;
use tune::pitch::Ratio;
use tune::scala::KbmRoot;
use tune::tuning::Scale;
use tune_cli::shared::midi::TuningMethod;

use crate::app::input::MenuMode;
use crate::app::resources::virtual_keyboard::OnScreenKeyboards;
use crate::app::resources::MainViewResource;
use crate::app::resources::MenuStackResource;
use crate::app::resources::PianoEngineResource;
use crate::app::resources::PianoEngineStateResource;
use crate::app::resources::PipelineEventsResource;
use crate::app::view::on_screen_keyboard::KeyboardCreator;
use crate::app::view::on_screen_keyboard::OnScreenKeyboard;
use crate::app::PipelineEvent;
use crate::app::VirtualKeyboardResource;
use crate::control::LiveParameter;
use crate::piano::PianoEngineState;
use crate::pipeline::NoAudioEvent;
use crate::tunable;

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
    pub const MENU_TEXT: f32 = 0.1;
    pub const PITCH_LINE: f32 = 0.2;
    pub const PITCH_TEXT: f32 = 0.3;
    pub const DEVIATION_MARKER: f32 = 0.4;
    pub const DEVIATION_TEXT: f32 = 0.5;
}

const FONT_RESOLUTION: f32 = 30.0;

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Srgba::hex("222222").unwrap().into()))
            .insert_resource(PipelineAggregate::default())
            .add_systems(Startup, (init_scene, init_menu, init_recording_indicator))
            .add_systems(
                Update,
                (
                    (process_updates, handle_pipeline_events),
                    press_keys,
                    update_menu,
                    update_recording_indicator,
                )
                    .chain(),
            );
    }
}

#[derive(Resource, Default)]
struct PipelineAggregate {
    backend_title: &'static str,
    backend_details: String,
    recorder_details: BTreeMap<usize, String>,
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

fn process_updates(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    engine: Res<PianoEngineResource>,
    mut state: ResMut<PianoEngineStateResource>,
    virtual_keyboard: Res<VirtualKeyboardResource>,
    main_view: Res<MainViewResource>,
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    mut keys: Query<&mut Transform>,
    grid_lines: Query<Entity, With<GridLines>>,
    pitch_lines: Query<Entity, With<PitchLines>>,
) {
    press_or_lift_keys(&state.0, &keyboards, &mut keys, -1.0);

    engine.0.capture_state(&mut state.0);

    let scene_rerender_required =
        state.0.tuning_updated || virtual_keyboard.is_changed() || main_view.is_changed();
    let pitch_lines_rerender_required = state.0.keys_updated || scene_rerender_required;

    if scene_rerender_required {
        // Remove old keyboards
        for (entity, _) in &keyboards {
            commands.entity(entity).despawn_recursive();
        }

        create_keyboards(
            &mut commands,
            &mut meshes,
            &mut materials,
            &state.0,
            &virtual_keyboard,
            &main_view,
        );

        // Remove old grid lines
        for entity in &grid_lines {
            commands.entity(entity).despawn_recursive();
        }

        create_grid_lines(
            &mut commands,
            &mut meshes,
            &mut materials,
            &state.0,
            &main_view,
        );
    }

    if pitch_lines_rerender_required {
        // Remove old pitch lines
        for entity in &pitch_lines {
            commands.entity(entity).despawn_recursive();
        }

        create_pitch_lines_and_deviation_markers(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            &state.0,
            &main_view,
        );
    }
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    virtual_keyboard: &VirtualKeyboardResource,
    main_view: &MainViewResource,
) {
    fn get_12edo_key_color(key: i32) -> Srgba {
        if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
            css::WHITE * 0.2
        } else {
            css::WHITE
        }
    }

    let kbm_root = state.kbm.kbm_root();

    let (reference_keyboard_location, scale_keyboard_location, keyboard_location) =
        match virtual_keyboard.on_screen_keyboard.curr_option() {
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
        main_view,
        height: SCENE_HEIGHT_3D / 3.0 * KEYBOARD_VERT_FILL,
        width: 1.0,
    };

    if let Some(reference_keyboard_location) = reference_keyboard_location {
        creator.create_linear(
            (
                main_view.reference_scl.clone(),
                KbmRoot::from(Note::from_piano_key(kbm_root.ref_key)),
            ),
            |key| get_12edo_key_color(key + kbm_root.ref_key.midi_number()),
            reference_keyboard_location * SCENE_HEIGHT_3D,
        );
    }

    let colors = &virtual_keyboard.colors();
    let get_key_color =
        |key| colors[usize::from(math::i32_rem_u(key, u16::try_from(colors.len()).unwrap()))];

    if let Some(scale_keyboard_location) = scale_keyboard_location {
        creator.create_linear(
            (state.scl.clone(), kbm_root),
            get_key_color,
            scale_keyboard_location * SCENE_HEIGHT_3D,
        );
    }

    if let Some(keyboard_location) = keyboard_location {
        creator.create_isomorphic(
            virtual_keyboard,
            (state.scl.clone(), kbm_root),
            get_key_color,
            keyboard_location * SCENE_HEIGHT_3D,
        );
    }
}

fn press_keys(
    state: Res<PianoEngineStateResource>,
    keyboards: Query<(Entity, &mut OnScreenKeyboard)>,
    mut keys: Query<&mut Transform>,
) {
    press_or_lift_keys(&state.0, &keyboards, &mut keys, 1.0);
}

fn press_or_lift_keys(
    state: &PianoEngineState,
    keyboards: &Query<(Entity, &mut OnScreenKeyboard)>,
    keys: &mut Query<&mut Transform>,
    direction: f32,
) {
    for (_, keyboard) in keyboards {
        for pressed_key in state.pressed_keys.values().flatten() {
            for (key, amount) in keyboard.get_keys(pressed_key.pitch) {
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

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    main_view: &MainViewResource,
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

    let tuning = (&state.scl, state.kbm.kbm_root());
    for (degree, pitch_coord) in iterate_grid_coords(main_view, &tuning) {
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

fn create_pitch_lines_and_deviation_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    state: &PianoEngineState,
    main_view: &MainViewResource,
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

    let octave_range = main_view.pitch_range().as_octaves();

    let mut freqs_hz = state
        .pressed_keys
        .values()
        .flatten()
        .map(|key_info| key_info.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = freqs_hz.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = main_view.hor_world_coord(*second) as f32;

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
                Anchor::CenterLeft,
                Transform::from_xyz(pitch_coord, curr_line_center, z_index::PITCH_TEXT).with_scale(
                    Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION / LINE_TO_CHARACTER_RATIO),
                ),
            ));
        });

        curr_line_center -= LINE_HEIGHT;

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(main_view.odd_limit);

            let width = (approximation.deviation.as_octaves() / octave_range) as f32;

            let color = if width > 0.0 { css::GREEN } else { css::MAROON };

            scale_grid_canvas.with_children(|commands| {
                let mut transform = Transform::from_xyz(
                    pitch_coord - width / 2.0,
                    curr_line_center,
                    z_index::DEVIATION_MARKER,
                );
                commands.spawn((
                    Mesh2d(square_mesh.clone()),
                    MeshMaterial2d(color_materials.add(ColorMaterial::from_color(color))),
                    transform.with_scale(Vec3::new(width.abs(), LINE_HEIGHT, 0.0)),
                ));
                transform.translation.z = z_index::DEVIATION_TEXT;
                commands.spawn((
                    Text2d::new(format!(
                        "{}/{} [{:.0}c]",
                        approximation.numer,
                        approximation.denom,
                        approximation.deviation.as_cents().abs()
                    )),
                    TextFont::from_font_size(FONT_RESOLUTION),
                    TextColor(Color::WHITE),
                    Anchor::CenterLeft,
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
    main_view: &'a MainViewResource,
    tuning: &'a impl Scale,
) -> impl Iterator<Item = (i32, f32)> + 'a {
    tunable::range(tuning, main_view.viewport_left, main_view.viewport_right).map(
        move |key_degree| {
            (
                key_degree,
                main_view.hor_world_coord(tuning.sorted_pitch_of(key_degree)) as f32,
            )
        },
    )
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
                            "Recording buffers {} and {} into {}\n",
                            event.in_buffers.0, event.in_buffers.1, file_name
                        ),
                    );
                }
                None => {
                    aggregate.recorder_details.remove(&event.index);
                }
            },
            PipelineEvent::Magnetron(event) => {
                aggregate.backend_title = "Magnetron";
                aggregate.backend_details = format!(
                    "[Up/Down] Waveform: {waveform_number} - {waveform_name}\n\
                     [Alt+E] Envelope: {envelope_name}{is_default_indicator}\n",
                    waveform_number = event.waveform_number,
                    waveform_name = event.waveform_name,
                    envelope_name = event.envelope_name,
                    is_default_indicator = if event.is_default_envelope {
                        ""
                    } else {
                        " (default) "
                    }
                );
            }
            PipelineEvent::Fluid(event) => {
                aggregate.backend_title = "Fluid";
                aggregate.backend_details = format!(
                    "Soundfont File: {soundfont_file}\n\
                     Tuning method: {tuning_method}\n\
                     [Up/Down] Program: {program}\n",
                    soundfont_file = event.soundfont_location,
                    tuning_method = match event.is_tuned {
                        true => "Single Note Tuning Change",
                        false => "None. Tuning channels exceeded! Change tuning mode.",
                    },
                    program = OptionFormatter(
                        event
                            .program
                            .map(|(number, name)| format!("{number} - {name}",))
                    )
                );
            }
            PipelineEvent::FluidError(error) => {
                aggregate.backend_title = "Fluid";
                aggregate.backend_details = format!(
                    "Soundfont File: {soundfont_file}\n\
                     Error: {error_message}\n",
                    soundfont_file = error.soundfont_location,
                    error_message = error.error_message,
                );
            }
            PipelineEvent::MidiOut(event) => {
                aggregate.backend_title = "MIDI Out";
                aggregate.backend_details = format!(
                    "Device: {device}\n\
                     Tuning method: {tuning_method}\n\
                     [PgUp/PgDown] Bank: {bank_msb}/{bank_lsb}\n\
                     [Up/Down] Program: {program_number}\n",
                    device = event.device,
                    bank_msb = OptionFormatter(event.bank_msb),
                    bank_lsb = OptionFormatter(event.bank_lsb),
                    tuning_method = match event.tuning_method {
                        Some(TuningMethod::FullKeyboard) => "Single Note Tuning Change",
                        Some(TuningMethod::FullKeyboardRt) =>
                            "Single Note Tuning Change (real-time)",
                        Some(TuningMethod::Octave1) => "Scale/Octave Tuning (1-Byte)",
                        Some(TuningMethod::Octave1Rt) => "Scale/Octave Tuning (1-Byte) (real-time)",
                        Some(TuningMethod::Octave2) => "Scale/Octave Tuning (2-Byte)",
                        Some(TuningMethod::Octave2Rt) => "Scale/Octave Tuning (2-Byte) (real-time)",
                        Some(TuningMethod::ChannelFineTuning) => "Channel Fine Tuning",
                        Some(TuningMethod::PitchBend) => "Pitch Bend",
                        None => "None. Tuning channels exceeded! Change tuning mode.",
                    },
                    program_number = event.program_number,
                );
            }
            PipelineEvent::MidiOutError(error) => {
                aggregate.backend_title = "MIDI Out";
                aggregate.backend_details = format!(
                    "Device: {device}\n\
                    Error: {error_message}\n",
                    device = error.out_device,
                    error_message = error.error_message,
                );
            }
            PipelineEvent::NoAudio(NoAudioEvent) => {
                aggregate.backend_title = "No Audio";
                aggregate.backend_details.clear();
            }
        }
    }
}

#[derive(Component)]
struct Menu;

fn init_menu(mut commands: Commands, assets: Res<AssetServer>) {
    const LINE_HEIGHT: f32 = calc_font_height(45);

    commands.spawn((
        Menu,
        Text2d::new("translation"),
        TextFont::from_font_size(FONT_RESOLUTION).with_font(assets.load("FiraMono-Regular.ttf")),
        TextColor(css::LIME.into()),
        Anchor::TopLeft,
        compress_text(
            Transform::from_xyz(SCENE_LEFT, SCENE_TOP_2D, z_index::MENU_TEXT)
                .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION)),
        ),
    ));
}

fn update_menu(
    mut menus: Query<&mut Text2d, With<Menu>>,
    aggregate: Res<PipelineAggregate>,
    state: Res<PianoEngineStateResource>,
    menu_stack: Res<MenuStackResource>,
    virtual_keyboard: Res<VirtualKeyboardResource>,
    main_view: Res<MainViewResource>,
) {
    for mut menu in &mut menus {
        menu.clear();

        match menu_stack.top() {
            None => {
                writeln!(
                    menu,
                    "Scale: {}\n\
                     \n\
                     [Alt+Left/Right] Reference note: {}\n\
                     [Left/Right] Scale offset: {:+}\n\
                     [Alt+Up/Down] Output target: {}",
                    state.0.scl.description(),
                    state.0.kbm.kbm_root().ref_key.midi_number(),
                    state.0.kbm.kbm_root().root_offset,
                    aggregate.backend_title,
                )
                .unwrap();

                menu.push_str(&aggregate.backend_details);

                let effects = [
                    LiveParameter::Sound1,
                    LiveParameter::Sound2,
                    LiveParameter::Sound3,
                    LiveParameter::Sound4,
                    LiveParameter::Sound5,
                    LiveParameter::Sound6,
                    LiveParameter::Sound7,
                    LiveParameter::Sound8,
                    LiveParameter::Sound9,
                    LiveParameter::Sound10,
                ]
                .into_iter()
                .enumerate()
                .filter(|&(_, p)| state.0.storage.is_active(p))
                .map(|(i, p)| format!("{} (cc {})", i + 1, state.0.mapper.get_ccn(p).unwrap()))
                .collect::<Vec<_>>()
                .join(", ");

                writeln!(
                    menu,
                    "[Alt+T] Tuning mode: {:?}\n\
                     [Alt+L] Legato: {}\n\
                     [F1-F10] Effects: {}\n\
                     [Alt+K] Keyboard settings ...\n\
                     [(Alt+)Scroll] Range: {:.0}..{:.0} Hz\n",
                    state.0.tuning_mode,
                    if state.0.storage.is_active(LiveParameter::Legato) {
                        format!(
                            "ON (cc {})",
                            state.0.mapper.get_ccn(LiveParameter::Legato).unwrap()
                        )
                    } else {
                        "OFF".to_owned()
                    },
                    effects,
                    main_view.viewport_left.as_hz(),
                    main_view.viewport_right.as_hz(),
                )
                .unwrap();

                for recorder_detail in aggregate.recorder_details.values() {
                    menu.push_str(recorder_detail);
                }
            }
            Some(MenuMode::Keyboard) => {
                let scale_steps = virtual_keyboard.scale_step_sizes();
                let layout_steps = virtual_keyboard.layout_step_sizes();

                writeln!(
                    menu,
                    "MOS scale: primary_step = {} | secondary_step = {} | sharpness = {}\n\
                     Isomorphic layout: east = {} | south-east = {} | north-east = {}\n\
                     \n\
                     [Alt+K] On-screen keyboards: {}\n\
                     [Alt+S] Scale: {}\n\
                     [Alt+L] Layout: {}\n\
                     [Alt+C] Compression: {:?}\n\
                     [Alt+T] Tilt: {:?}\n\
                     [Alt+I] Inclination: {:?}\n\
                     \n\
                     [Esc] Back",
                    scale_steps.0,
                    scale_steps.1,
                    scale_steps.2,
                    layout_steps.0,
                    layout_steps.1,
                    layout_steps.2,
                    virtual_keyboard.on_screen_keyboard.curr_option(),
                    virtual_keyboard.scale_name(),
                    virtual_keyboard.layout_name(),
                    virtual_keyboard.compression.curr_option(),
                    virtual_keyboard.tilt.curr_option(),
                    virtual_keyboard.inclination.curr_option(),
                )
                .unwrap();
            }
        }
    }
}

struct OptionFormatter<T>(Option<T>);

impl<T: Display> Display for OptionFormatter<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{}", value),
            None => write!(f, "-"),
        }
    }
}

#[derive(Component)]
struct RecordingIndicator;

fn init_recording_indicator(
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

fn update_recording_indicator(
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
