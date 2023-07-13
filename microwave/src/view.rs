use core::f32;
use std::{
    f32::consts,
    fmt::{self, Write},
    ops::RangeInclusive,
};

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    ecs::system::EntityCommands,
    prelude::{
        shape::{Circle, Cube, Quad},
        *,
    },
    render::{camera::ScalingMode, render_resource::PrimitiveTopology},
    sprite::{Anchor, MaterialMesh2dBundle},
};
use crossbeam::channel::Receiver;
use tune::{
    math,
    note::Note,
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
    tuning::Scale,
};
use tune_cli::shared::midi::TuningMethod;

use crate::{
    control::LiveParameter,
    fluid::{FluidError, FluidInfo},
    midi::{MidiOutError, MidiOutInfo},
    model::Viewport,
    piano::{PianoEngineEvent, PianoEngineState},
    profile::NoAudioInfo,
    synth::MagnetronInfo,
    tunable, KeyColor, Model,
};

const SCENE_HEIGHT_2D: f32 = 1.0 / 2.0; // Designed for 2:1 viewport ratio
const SCENE_BOTTOM_2D: f32 = -SCENE_HEIGHT_2D / 2.0;
const SCENE_TOP_2D: f32 = SCENE_HEIGHT_2D / 2.0;
const SCENE_HEIGHT_3D: f32 = SCENE_HEIGHT_2D * consts::SQRT_2; // 45-degree ortho perspective
const SCENE_BOTTOM_3D: f32 = -SCENE_HEIGHT_3D / 2.0;
const SCENE_TOP_3D: f32 = SCENE_HEIGHT_3D / 2.0;
const SCENE_LEFT: f32 = -0.5;

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
        app.insert_resource(DynViewInfo::from(NoAudioInfo))
            .insert_resource(FontResource(default()))
            .insert_resource(RequiredUpdates::OverlaysOnly)
            .add_systems(
                Startup,
                (load_font, init_scene, init_recording_indicator, init_hud),
            )
            .add_systems(
                Update,
                (
                    // There seems to be a bug in Bevy 0.11.0: Whenever a system is executed in parallel with a chain of systems, the chain no longer behaves correctly. As a workaround, the following tuple system is added to the chain even though it would be better, performance-wise, to execute it in parallel.
                    // TODO: Remove this workaround once the issue is resolved.
                    (update_recording_indicator, update_hud),
                    evaluate_required_updates,
                    update_scene,
                    apply_deferred,
                    update_scene_objects,
                )
                    .chain(),
            );
    }
}

#[derive(Resource)]
pub struct EventReceiver<T>(pub Receiver<T>);

#[derive(Resource)]
pub struct DynViewInfo(Box<dyn ViewModel>);

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
        camera_2d: Camera2d {
            clear_color: ClearColorConfig::None,
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
            intensity: 10.0,
            ..default()
        },
        transform,
        ..default()
    });
}

#[derive(Resource)]
pub struct PianoEngineResource(pub PianoEngineState);

#[derive(Resource, PartialEq, Eq, PartialOrd, Ord)]
enum RequiredUpdates {
    OverlaysOnly,
    PressedKeys,
    EntireScene,
}

fn evaluate_required_updates(
    viewport: Res<Viewport>,
    redraw_events: Res<EventReceiver<PianoEngineEvent>>,
    mut required_updates: ResMut<RequiredUpdates>,
) {
    *required_updates = redraw_events
        .0
        .try_iter()
        .map(|redraw_event| match redraw_event {
            PianoEngineEvent::UpdateScale => RequiredUpdates::EntireScene,
            PianoEngineEvent::UpdatePressedKeys => RequiredUpdates::PressedKeys,
        })
        .chain(
            viewport
                .is_changed()
                .then_some(RequiredUpdates::EntireScene),
        )
        .max()
        .unwrap_or(RequiredUpdates::OverlaysOnly)
}

type CurrentSceneQuery<'w, 's> = Query<'w, 's, Entity, Or<(With<LinearKeyboard>, With<GridLines>)>>;

#[allow(clippy::too_many_arguments)]
fn update_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    required_updates: Res<RequiredUpdates>,
    mut state: ResMut<PianoEngineResource>,
    viewport: Res<Viewport>,
    model: Res<Model>,
    current_scene: CurrentSceneQuery,
    current_keyboard: Query<(&mut LinearKeyboard, &Children)>,
    mut key_query: Query<&mut Transform>,
) {
    match *required_updates {
        RequiredUpdates::OverlaysOnly => {
            model.engine.capture_state(&mut state.0);
        }
        RequiredUpdates::PressedKeys => {
            // Lift currently pressed keys
            for (keyboard, keys) in &current_keyboard {
                for (key_index, _) in keyboard.pressed_keys(&state.0) {
                    reset_key(&mut key_query.get_mut(keys[key_index]).unwrap());
                }
            }

            model.engine.capture_state(&mut state.0);
        }
        RequiredUpdates::EntireScene => {
            // Remove old keyboards and grid lines
            for entity in &current_scene {
                commands.entity(entity).despawn_recursive();
            }

            model.engine.capture_state(&mut state.0);

            // Create new keyboards
            create_keyboards(
                &mut commands,
                &mut meshes,
                &mut materials,
                &state.0,
                &viewport,
                &model,
            );

            // Create new grid lines
            create_grid_lines(
                &mut commands,
                &mut meshes,
                &mut materials,
                &state.0,
                &viewport,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_scene_objects(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    required_updates: Res<RequiredUpdates>,
    state: ResMut<PianoEngineResource>,
    viewport: Res<Viewport>,
    model: Res<Model>,
    current_keyboards: Query<(&mut LinearKeyboard, &Children)>,
    mut current_keys: Query<&mut Transform>,
    current_pitch_lines: Query<Entity, With<PitchLines>>,
    font: Res<FontResource>,
) {
    match *required_updates {
        RequiredUpdates::OverlaysOnly => {}
        RequiredUpdates::PressedKeys | RequiredUpdates::EntireScene => {
            // Press keys
            for (keyboard, keys) in &current_keyboards {
                for (key_index, amount) in keyboard.pressed_keys(&state.0) {
                    press_key(&mut current_keys.get_mut(keys[key_index]).unwrap(), amount);
                }
            }

            // Remove old pitch lines
            for entity in &current_pitch_lines {
                commands.entity(entity).despawn_recursive();
            }

            // Create new grid lines
            create_pitch_lines_and_deviation_markers(
                &mut commands,
                &mut meshes,
                &mut color_materials,
                &state.0,
                &viewport,
                &model,
                &font.0,
            );
        }
    }
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    viewport: &Viewport,
    model: &Model,
) {
    fn get_12edo_key_color(key: i32) -> KeyColor {
        if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
            KeyColor::Black
        } else {
            KeyColor::White
        }
    }

    let kbm_root = state.kbm.kbm_root();

    create_keyboard(
        commands,
        meshes,
        materials,
        viewport,
        (
            &model.reference_scl,
            KbmRoot::from(Note::from_piano_key(kbm_root.ref_key)),
        ),
        |key| get_12edo_key_color(key + kbm_root.ref_key.midi_number()),
    )
    .insert(Transform::from_xyz(0.0, 0.0, SCENE_HEIGHT_3D / 3.0));

    let render_second_keyboard = !model.key_colors.is_empty();
    if render_second_keyboard {
        create_keyboard(
            commands,
            meshes,
            materials,
            viewport,
            (&state.scl, kbm_root),
            |key| {
                model.key_colors[Into::<usize>::into(math::i32_rem_u(
                    key,
                    u16::try_from(model.key_colors.len()).unwrap(),
                ))]
            },
        )
        .insert(Transform::from_xyz(0.0, 0.0, 0.0));
    }
}

#[derive(Component)]
struct LinearKeyboard {
    scl: Scl,
    kbm_root: KbmRoot,
    key_range: RangeInclusive<i32>,
}

fn create_keyboard<'w, 's, 'a>(
    commands: &'a mut Commands<'w, 's>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    viewport: &Viewport,
    tuning: (&Scl, KbmRoot),
    get_key_color: impl Fn(i32) -> KeyColor,
) -> EntityCommands<'w, 's, 'a> {
    let (key_range, grid_coords) = iterate_grid_coords(viewport, tuning, 1);

    let mut keyboard = commands.spawn((
        LinearKeyboard {
            scl: tuning.0.clone(),
            kbm_root: tuning.1,
            key_range,
        },
        SpatialBundle::default(),
    ));
    let mesh = meshes.add(Cube::default().into());

    let (mut mid, mut right) = Default::default();
    for (iterated_key, pitch_coord) in grid_coords {
        let left = mid;
        mid = right;
        right = Some(pitch_coord);

        if let (Some(left), Some(mid), Some(right)) = (left, mid, right) {
            let drawn_key = iterated_key - 1;
            let key_center = (left + right) / 4.0 + mid / 2.0;
            let key_width = ((right - left) / 2.0).max(0.0);

            let key_box_size = Vec3::new(
                0.9 * key_width,
                0.5 * key_width,
                SCENE_HEIGHT_3D / 3.0 * 0.85,
            );

            let mut transform = Transform::from_scale(key_box_size);
            transform.translation.x = key_center;
            reset_key(&mut transform);

            let key_color = match get_key_color(drawn_key) {
                KeyColor::White => Color::WHITE,
                KeyColor::Black => Color::BLACK,
                KeyColor::Red => Color::MAROON,
                KeyColor::Green => Color::DARK_GREEN,
                KeyColor::Blue => Color::BLUE,
                KeyColor::Cyan => Color::TEAL,
                KeyColor::Magenta => Color::rgb(0.5, 0.0, 1.0),
                KeyColor::Yellow => Color::OLIVE,
            };

            keyboard.with_children(|commands| {
                let mut key = commands.spawn(MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: materials.add(StandardMaterial {
                        base_color: key_color,
                        perceptual_roughness: 0.0,
                        metallic: 1.5,
                        ..default()
                    }),
                    transform,
                    ..default()
                });

                let draw_key_marker = drawn_key == 0;
                if draw_key_marker {
                    let key_box_size = transform.scale;
                    let key_width = key_box_size.x / 0.9;
                    let size_offset_to_reach_full_width = key_width - key_box_size.x;
                    let marker_box_size = key_box_size + size_offset_to_reach_full_width;
                    let marker_box_size_relative_to_parent = marker_box_size / key_box_size;

                    key.with_children(|commands| {
                        commands.spawn(MaterialMeshBundle {
                            mesh: mesh.clone(),
                            material: materials.add(StandardMaterial {
                                base_color: Color::rgba(1.0, 0.0, 0.0, 0.5),
                                alpha_mode: AlphaMode::Blend,
                                perceptual_roughness: 0.0,
                                metallic: 1.5,
                                ..default()
                            }),
                            transform: Transform::from_scale(marker_box_size_relative_to_parent),
                            ..default()
                        });
                    });
                }
            });
        }
    }

    keyboard
}

impl LinearKeyboard {
    fn pressed_keys<'a>(
        &'a self,
        state: &'a PianoEngineState,
    ) -> impl Iterator<Item = (usize, f64)> + 'a {
        state
            .pressed_keys
            .values()
            .flat_map(|pressed_key| self.get_interpolated_key_indexes(pressed_key.pitch))
            .flatten()
    }

    fn get_interpolated_key_indexes(&self, pitch: Pitch) -> [Option<(usize, f64)>; 2] {
        // Matching precise pitches is broken due to https://github.com/rust-lang/rust/issues/107904.
        let pitch = pitch * Ratio::from_float(0.999999);

        let tuning = (&self.scl, self.kbm_root);
        let approximation = tuning.find_by_pitch_sorted(pitch);
        let deviation_from_closest = approximation.deviation.as_octaves();

        let closest_degree = approximation.approx_value;
        let second_closest_degree = if deviation_from_closest < 0.0 {
            closest_degree - 1
        } else {
            closest_degree + 1
        };

        let second_closest_pitch = tuning.sorted_pitch_of(second_closest_degree);
        let deviation_from_second_closest =
            Ratio::between_pitches(pitch, second_closest_pitch).as_octaves();

        let interpolation = deviation_from_second_closest
            / (deviation_from_closest + deviation_from_second_closest);

        [
            self.to_interpolated_index(closest_degree, interpolation),
            self.to_interpolated_index(second_closest_degree, 1.0 - interpolation),
        ]
    }

    fn to_interpolated_index(
        &self,
        sorted_degree: i32,
        interpolation: f64,
    ) -> Option<(usize, f64)> {
        (self.key_range).contains(&sorted_degree).then(|| {
            (
                usize::try_from(sorted_degree - self.key_range.start()).unwrap(),
                interpolation,
            )
        })
    }
}

fn reset_key(transform: &mut Transform) {
    transform.translation.y = -transform.scale.y / 2.0;
    transform.translation.z = 0.0;
    transform.rotation = Quat::default();
}

fn press_key(transform: &mut Transform, amount: f64) {
    transform.rotate_around(
        Vec3::NEG_Z * transform.scale.z,
        Quat::from_rotation_x(1.5 * amount as f32 * consts::TAU / 360.0),
    );
}

#[derive(Component)]
struct GridLines;

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    viewport: &Viewport,
) {
    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip);
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
    for (degree, pitch_coord) in iterate_grid_coords(viewport, tuning, 0).1 {
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

fn iterate_grid_coords<'a>(
    viewport: &'a Viewport,
    tuning: (&'a Scl, KbmRoot),
    padding: i32,
) -> (RangeInclusive<i32>, impl Iterator<Item = (i32, f32)> + 'a) {
    let range = tunable::range(tuning, viewport.pitch_range.start, viewport.pitch_range.end);
    let padded_range = range.start() - padding..=range.end() + padding;
    let octave_range =
        Ratio::between_pitches(viewport.pitch_range.start, viewport.pitch_range.end).as_octaves();

    (
        range,
        padded_range.map(move |key_degree| {
            let pitch = tuning.sorted_pitch_of(key_degree);
            let pitch_coord = (Ratio::between_pitches(viewport.pitch_range.start, pitch)
                .as_octaves()
                / octave_range) as f32
                - 0.5;
            (key_degree, pitch_coord)
        }),
    )
}

#[derive(Component)]
struct PitchLines;

fn create_pitch_lines_and_deviation_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    state: &PianoEngineState,
    viewport: &Viewport,
    model: &Model,
    font: &Handle<Font>,
) {
    const LINE_HEIGHT: f32 = SCENE_HEIGHT_2D / 24.0;
    const FIRST_LINE_CENTER: f32 = SCENE_TOP_2D - LINE_HEIGHT / 2.0;

    let mut scale_grid_canvas = commands.spawn((PitchLines, SpatialBundle::default()));

    let line_mesh = meshes.add({
        let mut mesh = Mesh::new(PrimitiveTopology::LineStrip);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                Vec3::new(0.0, SCENE_BOTTOM_2D, 0.0),
                Vec3::new(0.0, SCENE_TOP_2D, 0.0),
            ],
        );
        mesh
    });

    let square_mesh = meshes.add(Quad::default().into());

    let octave_range =
        Ratio::between_pitches(viewport.pitch_range.start, viewport.pitch_range.end).as_octaves();

    let mut freqs_hz = state
        .pressed_keys
        .values()
        .filter(|pressed_key| !pressed_key.shadow)
        .map(|pressed_key| pressed_key.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = freqs_hz.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = (Ratio::between_pitches(viewport.pitch_range.start, *second).as_octaves()
            / octave_range) as f32
            - 0.5;

        scale_grid_canvas.with_children(|commands| {
            commands.spawn(MaterialMesh2dBundle {
                mesh: line_mesh.clone().into(),
                transform: Transform::from_xyz(pitch_coord, 0.0, z_index::PITCH_LINE),
                material: color_materials.add(Color::WHITE.into()),
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
                Ratio::between_pitches(*first, *second).nearest_fraction(model.odd_limit);

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
                    material: color_materials.add(color.into()),
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
            mesh: meshes.add(Circle::default().into()).into(),
            transform: Transform::from_xyz(0.5 - 0.05, 0.25 - 0.05, z_index::RECORDING_INDICATOR)
                .with_scale(Vec3::splat(0.05)),
            material: materials.add(Color::RED.into()),
            visibility: Visibility::Visible,
            ..default()
        },
    ));
}

fn update_recording_indicator(
    mut query: Query<&mut Visibility, With<RecordingIndicator>>,
    state: Res<PianoEngineResource>,
) {
    let recording_active = state.0.storage.is_active(LiveParameter::Foot);
    for mut visibility in &mut query {
        let target_visibility = if recording_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != target_visibility {
            *visibility = target_visibility
        }
    }
}

#[derive(Component)]
struct Hud;

fn init_hud(mut commands: Commands) {
    const LINE_HEIGHT: f32 = SCENE_HEIGHT_2D / 36.0;

    commands.spawn((
        Hud,
        Text2dBundle {
            text: default(),
            text_anchor: Anchor::TopLeft,
            transform: Transform::from_xyz(SCENE_LEFT, SCENE_TOP_2D, z_index::HUD_TEXT)
                .with_scale(Vec3::splat(LINE_HEIGHT / FONT_RESOLUTION)),
            ..default()
        },
    ));
}

fn update_hud(
    mut query: Query<&mut Text, With<Hud>>,
    font: Res<FontResource>,
    state: Res<PianoEngineResource>,
    mut info: ResMut<DynViewInfo>,
    info_updates: Res<EventReceiver<DynViewInfo>>,
    viewport: Res<Viewport>,
) {
    for info_update in info_updates.0.try_iter() {
        *info = info_update;
    }
    for mut hud_text in &mut query {
        let current_text = &hud_text.sections.get(0).map(|section| &section.value);
        let new_text = create_hud_text(&state.0, &viewport, &info);
        if &Some(&new_text) != current_text {
            *hud_text = Text::from_section(
                new_text,
                TextStyle {
                    font: font.0.clone(),
                    font_size: FONT_RESOLUTION,
                    color: Color::GREEN,
                },
            )
        }
    }
}

fn create_hud_text(state: &PianoEngineState, viewport: &Viewport, info: &DynViewInfo) -> String {
    let mut hud_text = String::new();

    writeln!(
        hud_text,
        "Scale: {scale}\n\
         Reference Note [Alt+Left/Right]: {ref_note}\n\
         Scale Offset [Left/Right]: {offset:+}\n\
         Output target [Alt+O]: {target}",
        scale = state.scl.description(),
        ref_note = state.kbm.kbm_root().ref_key.midi_number(),
        offset = state.kbm.kbm_root().root_offset,
        target = info.0.description(),
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
    .collect::<Vec<_>>();

    writeln!(
        hud_text,
        "Tuning Mode [Alt+T]: {tuning_mode:?}\n\
         Legato [Alt+L]: {legato}\n\
         Effects [F1-F10]: {effects}\n\
         Recording [Space]: {recording}\n\
         Range [Alt+/Scroll]: {from:.0}..{to:.0} Hz",
        tuning_mode = state.tuning_mode,
        effects = effects.join(", "),
        legato = if state.storage.is_active(LiveParameter::Legato) {
            format!(
                "ON (cc {})",
                state.mapper.get_ccn(LiveParameter::Legato).unwrap()
            )
        } else {
            "OFF".to_owned()
        },
        recording = if state.storage.is_active(LiveParameter::Foot) {
            format!(
                "ON (cc {})",
                state.mapper.get_ccn(LiveParameter::Foot).unwrap()
            )
        } else {
            "OFF".to_owned()
        },
        from = viewport.pitch_range.start.as_hz(),
        to = viewport.pitch_range.end.as_hz(),
    )
    .unwrap();

    hud_text
}

pub trait ViewModel: Sync + Send + 'static {
    fn description(&self) -> &'static str;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}

impl<T: ViewModel> From<T> for DynViewInfo {
    fn from(data: T) -> Self {
        DynViewInfo(Box::new(data))
    }
}

impl ViewModel for MagnetronInfo {
    fn description(&self) -> &'static str {
        "Magnetron"
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Waveform [Up/Down]: {waveform_number} - {waveform_name}\n\
             Envelope [Alt+E]: {envelope_name}{is_default_indicator}",
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

impl ViewModel for FluidInfo {
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
             Program [Up/Down]: {program_number} - {program_name}",
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

impl ViewModel for FluidError {
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

impl ViewModel for MidiOutInfo {
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
             Program [Up/Down]: {program_number}",
            device = self.device,
            program_number = self.program_number,
        )
    }
}

impl ViewModel for MidiOutError {
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

impl ViewModel for NoAudioInfo {
    fn description(&self) -> &'static str {
        "No Audio"
    }

    fn write_info(&self, _target: &mut String) -> fmt::Result {
        Ok(())
    }
}
