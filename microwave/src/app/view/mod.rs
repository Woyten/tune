use core::f32;
use std::{
    f32::consts,
    fmt::{self, Write},
};

use bevy::{
    prelude::*,
    render::{camera::ScalingMode, render_resource::PrimitiveTopology},
    sprite::{Anchor, MaterialMesh2dBundle},
};
use tune::{math, note::Note, pitch::Ratio, scala::KbmRoot, tuning::Scale};
use tune_cli::shared::midi::TuningMethod;

use crate::{
    app::{
        input::HudMode,
        resources::{
            virtual_keyboard::OnScreenKeyboards, BackendInfoResource, HudStackResource,
            MainViewResource, PianoEngineResource, PianoEngineStateResource,
        },
        view::on_screen_keyboard::{KeyboardCreator, OnScreenKeyboard},
        BackendInfo, DynBackendInfo, VirtualKeyboardResource,
    },
    control::LiveParameter,
    fluid::{FluidError, FluidInfo},
    midi::{MidiOutError, MidiOutInfo},
    piano::PianoEngineState,
    profile::NoAudioInfo,
    synth::MagnetronInfo,
    tunable,
};

mod on_screen_keyboard;

const SCENE_HEIGHT_2D: f32 = 1.0 / 2.0; // Designed for 2:1 viewport ratio
const SCENE_BOTTOM_2D: f32 = -SCENE_HEIGHT_2D / 2.0;
const SCENE_TOP_2D: f32 = SCENE_HEIGHT_2D / 2.0;
const SCENE_HEIGHT_3D: f32 = SCENE_HEIGHT_2D * consts::SQRT_2; // 45-degree ortho perspective
const SCENE_BOTTOM_3D: f32 = -SCENE_HEIGHT_3D / 2.0;
const SCENE_TOP_3D: f32 = SCENE_HEIGHT_3D / 2.0;
const SCENE_LEFT: f32 = -0.5;
const KEYBOARD_VERT_FILL: f32 = 0.85;

mod z_index {
    pub const RECORDING_INDICATOR: f32 = 0.0;
    pub const HUD_TEXT: f32 = 0.1;
    pub const PITCH_LINE: f32 = 0.2;
    pub const PITCH_TEXT: f32 = 0.3;
    pub const DEVIATION_MARKER: f32 = 0.4;
    pub const DEVIATION_TEXT: f32 = 0.5;
}

const FONT_RESOLUTION: f32 = 60.0;

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::hex("222222").unwrap()))
            .insert_resource(DynBackendInfo::from(NoAudioInfo))
            .insert_resource(FontResource(default()))
            .add_systems(
                Startup,
                (load_font, init_scene, init_recording_indicator, init_hud),
            )
            .add_systems(
                Update,
                (
                    process_updates,
                    (press_keys, update_recording_indicator, update_hud),
                )
                    .chain(),
            );
    }
}

#[derive(Resource)]
struct FontResource(Handle<Font>);

fn load_font(asset_server: Res<AssetServer>, mut font: ResMut<FontResource>) {
    *font = FontResource(asset_server.load("FiraSans-Regular.ttf"));
}

fn init_scene(mut commands: Commands) {
    create_3d_camera(&mut commands);
    create_2d_camera(&mut commands);
    create_light(&mut commands, Transform::from_xyz(-0.25, 7.5, -7.5));
    create_light(&mut commands, Transform::from_xyz(0.25, 7.5, -7.5));
}

fn create_3d_camera(commands: &mut Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::NEG_Z),
        projection: Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal(1.0),
            ..default()
        }),
        camera: Camera {
            order: 0,
            ..default()
        },
        ..default()
    });
}

fn create_2d_camera(commands: &mut Commands) {
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 1.0),
        projection: OrthographicProjection {
            scaling_mode: ScalingMode::FixedHorizontal(1.0),
            ..default()
        },
        camera: Camera {
            order: 1,
            ..default()
        },
        ..default()
    });
}

fn create_light(commands: &mut Commands, transform: Transform) {
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 10000.0,
            ..default()
        },
        transform,
        ..default()
    });
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
    font: Res<FontResource>,
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
            &font.0,
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
    fn get_12edo_key_color(key: i32) -> Color {
        if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
            Color::WHITE * 0.2
        } else {
            Color::WHITE
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

    if let Some(scale_keyboard_location) = scale_keyboard_location {
        creator.create_linear(
            (state.scl.clone(), kbm_root),
            |key| colors[usize::from(math::i32_rem_u(key, u16::try_from(colors.len()).unwrap()))],
            scale_keyboard_location * SCENE_HEIGHT_3D,
        );
    }

    if let Some(keyboard_location) = keyboard_location {
        creator.create_isomorphic(
            virtual_keyboard,
            (state.scl.clone(), kbm_root),
            |key| colors[usize::from(math::i32_rem_u(key, u16::try_from(colors.len()).unwrap()))],
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

    let mut scale_grid = commands.spawn((GridLines, SpatialBundle::default()));

    let tuning = (&state.scl, state.kbm.kbm_root());
    for (degree, pitch_coord) in iterate_grid_coords(main_view, &tuning) {
        let line_color = match degree {
            0 => Color::SALMON,
            _ => Color::GRAY,
        };

        scale_grid.with_children(|commands| {
            commands.spawn(MaterialMeshBundle {
                mesh: line_mesh.clone(),
                transform: Transform::from_xyz(pitch_coord, -10.0, -10.0),
                material: materials.add(StandardMaterial {
                    base_color: line_color,
                    unlit: true,
                    ..default()
                }),
                ..default()
            });
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
    font: &Handle<Font>,
) {
    const LINE_HEIGHT: f32 = SCENE_HEIGHT_2D / 24.0;
    const FIRST_LINE_CENTER: f32 = SCENE_TOP_2D - LINE_HEIGHT / 2.0;

    let mut scale_grid_canvas = commands.spawn((PitchLines, SpatialBundle::default()));

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
            commands.spawn(MaterialMesh2dBundle {
                mesh: line_mesh.clone().into(),
                transform: Transform::from_xyz(pitch_coord, 0.0, z_index::PITCH_LINE),
                material: color_materials.add(Color::WHITE),
                ..default()
            });
        });

        let mut curr_line_center = FIRST_LINE_CENTER;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{:.0} Hz", second.as_hz()),
                    TextStyle {
                        font: font.clone(),
                        font_size: FONT_RESOLUTION,
                        color: Color::RED,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform::from_xyz(pitch_coord, curr_line_center, z_index::PITCH_TEXT)
                    .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION * 0.75)),
                ..default()
            });
        });

        curr_line_center -= LINE_HEIGHT;

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(main_view.odd_limit);

            let width = (approximation.deviation.as_octaves() / octave_range) as f32;

            let color = if width > 0.0 {
                Color::DARK_GREEN
            } else {
                Color::MAROON
            };

            scale_grid_canvas.with_children(|commands| {
                let mut transform = Transform::from_xyz(
                    pitch_coord - width / 2.0,
                    curr_line_center,
                    z_index::DEVIATION_MARKER,
                );
                commands.spawn(MaterialMesh2dBundle {
                    mesh: square_mesh.clone().into(),
                    transform: transform.with_scale(Vec3::new(width.abs(), LINE_HEIGHT, 0.0)),
                    material: color_materials.add(color),
                    ..default()
                });
                transform.translation.z = z_index::DEVIATION_TEXT;
                commands.spawn(Text2dBundle {
                    text: Text::from_section(
                        format!(
                            "{}/{} [{:.0}c]",
                            approximation.numer,
                            approximation.denom,
                            approximation.deviation.as_cents().abs()
                        ),
                        TextStyle {
                            font: font.clone(),
                            font_size: FONT_RESOLUTION,
                            color: Color::WHITE,
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: transform
                        .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION * 0.66)),
                    ..default()
                });
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

#[derive(Component)]
struct RecordingIndicator;

fn init_recording_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        RecordingIndicator,
        MaterialMesh2dBundle {
            mesh: meshes.add(Circle::default()).into(),
            transform: Transform::from_xyz(0.5 - 0.05, 0.25 - 0.05, z_index::RECORDING_INDICATOR)
                .with_scale(Vec3::splat(0.05)),
            material: materials.add(Color::RED),
            ..default()
        },
    ));
}

fn update_recording_indicator(
    mut recording_indicator_visibilities: Query<&mut Visibility, With<RecordingIndicator>>,
    state: Res<PianoEngineStateResource>,
) {
    let recording_active = state.0.storage.is_active(LiveParameter::Foot);
    for mut visibility in &mut recording_indicator_visibilities {
        *visibility = if recording_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[derive(Component)]
struct Hud;

fn init_hud(mut commands: Commands) {
    const LINE_HEIGHT: f32 = SCENE_HEIGHT_2D / 40.0;

    commands.spawn((
        Hud,
        Text2dBundle {
            text: Text::from_section("", default()),
            text_anchor: Anchor::TopLeft,
            transform: Transform::from_xyz(SCENE_LEFT, SCENE_TOP_2D, z_index::HUD_TEXT)
                .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION)),
            ..default()
        },
    ));
}

fn update_hud(
    info_updates: Res<BackendInfoResource>,
    mut info: ResMut<DynBackendInfo>,
    mut hud_texts: Query<&mut Text, With<Hud>>,
    font: Res<FontResource>,
    state: Res<PianoEngineStateResource>,
    hud_stack: Res<HudStackResource>,
    virtual_keyboard: Res<VirtualKeyboardResource>,
    main_view: Res<MainViewResource>,
) {
    for info_update in info_updates.0.try_iter() {
        *info = info_update;
    }
    for mut hud_text in &mut hud_texts {
        hud_text.sections[0] = TextSection::new(
            create_hud_text(&state.0, &hud_stack, &virtual_keyboard, &main_view, &info),
            TextStyle {
                font: font.0.clone(),
                font_size: FONT_RESOLUTION,
                color: Color::GREEN,
            },
        )
    }
}

fn create_hud_text(
    state: &PianoEngineState,
    hud_stack: &HudStackResource,
    virtual_keyboard: &VirtualKeyboardResource,
    main_view: &MainViewResource,
    info: &DynBackendInfo,
) -> String {
    let mut hud_text = String::new();

    match hud_stack.top() {
        None => {
            writeln!(
                hud_text,
                "Scale: {}\n\
                 \n\
                 [Alt+Left/Right] Reference note: {}\n\
                 [Left/Right] Scale offset: {:+}\n\
                 [Alt+Up/Down] Output target: {}",
                state.scl.description(),
                state.kbm.kbm_root().ref_key.midi_number(),
                state.kbm.kbm_root().root_offset,
                info.0.description(),
            )
            .unwrap();

            info.0.write_info(&mut hud_text).unwrap();

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
            .filter(|&(_, p)| state.storage.is_active(p))
            .map(|(i, p)| format!("{} (cc {})", i + 1, state.mapper.get_ccn(p).unwrap()))
            .collect::<Vec<_>>()
            .join(", ");

            writeln!(
                hud_text,
                "[Alt+T] Tuning mode: {:?}\n\
                 [Alt+L] Legato: {}\n\
                 [F1-F10] Effects: {}\n\
                 [Space] Recording: {}\n\
                 [Alt+K] Keyboard settings ...\n\
                 [(Alt+)Scroll] Range: {:.0}..{:.0} Hz",
                state.tuning_mode,
                if state.storage.is_active(LiveParameter::Legato) {
                    format!(
                        "ON (cc {})",
                        state.mapper.get_ccn(LiveParameter::Legato).unwrap()
                    )
                } else {
                    "OFF".to_owned()
                },
                effects,
                if state.storage.is_active(LiveParameter::Foot) {
                    format!(
                        "ON (cc {})",
                        state.mapper.get_ccn(LiveParameter::Foot).unwrap()
                    )
                } else {
                    "OFF".to_owned()
                },
                main_view.viewport_left.as_hz(),
                main_view.viewport_right.as_hz(),
            )
            .unwrap();
        }
        Some(HudMode::Keyboard) => {
            let scale_steps = virtual_keyboard.scale_step_sizes();
            let layout_steps = virtual_keyboard.layout_step_sizes();

            writeln!(
                hud_text,
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

    hud_text
}

impl<T: BackendInfo> From<T> for DynBackendInfo {
    fn from(data: T) -> Self {
        DynBackendInfo(Box::new(data))
    }
}

impl BackendInfo for MagnetronInfo {
    fn description(&self) -> &'static str {
        "Magnetron"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "[Up/Down] Waveform: {waveform_number} - {waveform_name}\n\
             [Alt+E] Envelope: {envelope_name}{is_default_indicator}",
            waveform_number = self.waveform_number,
            waveform_name = self.waveform_name,
            envelope_name = self.envelope_name,
            is_default_indicator = if self.is_default_envelope {
                ""
            } else {
                " (default) "
            }
        )
    }
}

impl BackendInfo for FluidInfo {
    fn description(&self) -> &'static str {
        "Fluid"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        let tuning_method = match self.is_tuned {
            true => "Single Note Tuning Change",
            false => "None. Tuning channels exceeded! Change tuning mode.",
        };

        writeln!(
            target,
            "Soundfont File: {soundfont_file}\n\
             Tuning method: {tuning_method}\n\
             [Up/Down] Program: {program_number} - {program_name}",
            soundfont_file = self.soundfont_location,
            program_number = self
                .program
                .map(|p| p.to_string())
                .as_deref()
                .unwrap_or("Unknown"),
            program_name = self.program_name.as_deref().unwrap_or("Unknown"),
        )
    }
}

impl BackendInfo for FluidError {
    fn description(&self) -> &'static str {
        "Fluid"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Soundfont File: {soundfont_file}\n\
             Error: {error_message}",
            soundfont_file = self.soundfont_location,
            error_message = self.error_message,
        )
    }
}

impl BackendInfo for MidiOutInfo {
    fn description(&self) -> &'static str {
        "MIDI"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        let tuning_method = match self.tuning_method {
            Some(TuningMethod::FullKeyboard) => "Single Note Tuning Change",
            Some(TuningMethod::FullKeyboardRt) => "Single Note Tuning Change (realtime)",
            Some(TuningMethod::Octave1) => "Scale/Octave Tuning (1-Byte)",
            Some(TuningMethod::Octave1Rt) => "Scale/Octave Tuning (1-Byte) (realtime)",
            Some(TuningMethod::Octave2) => "Scale/Octave Tuning (2-Byte)",
            Some(TuningMethod::Octave2Rt) => "Scale/Octave Tuning (2-Byte) (realtime)",
            Some(TuningMethod::ChannelFineTuning) => "Channel Fine Tuning",
            Some(TuningMethod::PitchBend) => "Pitch Bend",
            None => "None. Tuning channels exceeded! Change tuning mode.",
        };

        writeln!(
            target,
            "Device: {device}\n\
             Tuning method: {tuning_method}\n\
             [Up/Down] Program: {program_number}",
            device = self.device,
            program_number = self.program_number,
        )
    }
}

impl BackendInfo for MidiOutError {
    fn description(&self) -> &'static str {
        "MIDI"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Device: {device}\n\
            Error: {error_message}",
            device = self.out_device,
            error_message = self.error_message,
        )
    }
}

impl BackendInfo for NoAudioInfo {
    fn description(&self) -> &'static str {
        "No Audio"
    }

    fn write_info(&self, _target: &mut String) -> fmt::Result {
        Ok(())
    }
}
