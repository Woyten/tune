use core::f32;
use std::{
    f32::consts,
    fmt::{self, Write},
    ops::{Range, RangeInclusive},
};

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    prelude::{
        shape::{Circle, Cube, Cylinder, Quad},
        *,
    },
    render::{camera::ScalingMode, render_resource::PrimitiveTopology},
    sprite::{Anchor, MaterialMesh2dBundle},
};
use tune::{
    math,
    note::Note,
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
    tuning::Scale,
};
use tune_cli::shared::midi::TuningMethod;

use crate::{
    app::model::OnScreenKeyboards,
    control::LiveParameter,
    fluid::{FluidError, FluidInfo},
    midi::{MidiOutError, MidiOutInfo},
    piano::PianoEngineState,
    profile::NoAudioInfo,
    synth::MagnetronInfo,
    tunable, KeyColor,
};

use super::{
    model::{BackendInfoResource, PianoEngineResource, PianoEngineStateResource, ViewModel},
    BackendInfo, DynBackendInfo, VirtualKeyboardLayout,
};

const SCENE_HEIGHT_2D: f32 = 1.0 / 2.0; // Designed for 2:1 viewport ratio
const SCENE_BOTTOM_2D: f32 = -SCENE_HEIGHT_2D / 2.0;
const SCENE_TOP_2D: f32 = SCENE_HEIGHT_2D / 2.0;
const SCENE_HEIGHT_3D: f32 = SCENE_HEIGHT_2D * consts::SQRT_2; // 45-degree ortho perspective
const SCENE_BOTTOM_3D: f32 = -SCENE_HEIGHT_3D / 2.0;
const SCENE_TOP_3D: f32 = SCENE_HEIGHT_3D / 2.0;
const SCENE_LEFT: f32 = -0.5;
const SCENE_RIGHT: f32 = 0.5;
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
                    apply_deferred,
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

fn process_updates(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    engine: Res<PianoEngineResource>,
    mut state: ResMut<PianoEngineStateResource>,
    virtual_layout: Res<VirtualKeyboardLayout>,
    view_model: Res<ViewModel>,
    linear_keyboards: Query<(Entity, &mut LinearKeyboard, &Children)>,
    isomorphic_keyboards: Query<(Entity, &mut IsomorphicKeyboard, &Children)>,
    mut keys: Query<&mut Transform>,
    grid_lines: Query<Entity, With<GridLines>>,
    pitch_lines: Query<Entity, With<PitchLines>>,
    font: Res<FontResource>,
) {
    // Lift currently pressed keys
    for (_, keyboard, children) in &linear_keyboards {
        for (key_index, _) in keyboard.pressed_keys(&state.0) {
            reset_key(&mut keys.get_mut(children[key_index]).unwrap());
        }
    }

    engine.0.capture_state(&mut state.0);

    let scene_rerender_required = state.0.tuning_updated || view_model.is_changed();
    let pitch_lines_rerender_required = state.0.keys_updated || scene_rerender_required;

    if scene_rerender_required {
        // Remove old keyboards
        for (entity, _, _) in &linear_keyboards {
            commands.entity(entity).despawn_recursive();
        }

        // Remove old keyboards
        for (entity, _, _) in &isomorphic_keyboards {
            commands.entity(entity).despawn_recursive();
        }

        create_keyboards(
            &mut commands,
            &mut meshes,
            &mut materials,
            &state.0,
            &virtual_layout,
            &view_model,
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
            &view_model,
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
            &view_model,
            &font.0,
        );
    }
}

fn create_keyboards(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    virtual_layout: &VirtualKeyboardLayout,
    view_model: &ViewModel,
) {
    fn get_12edo_key_color(key: i32) -> KeyColor {
        if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
            KeyColor::Black
        } else {
            KeyColor::White
        }
    }

    let kbm_root = state.kbm.kbm_root();

    let (reference_keyboard_location, scale_keyboard_location, keyboard_location) =
        match view_model.on_screen_keyboards {
            OnScreenKeyboards::Isomorphic => (None, None, Some(1.0 / 3.0)),
            OnScreenKeyboards::Scale => (None, Some(1.0 / 3.0), None),
            OnScreenKeyboards::Reference => (Some(1.0 / 3.0), None, None),
            OnScreenKeyboards::IsomorphicAndReference => (Some(1.0 / 3.0), None, Some(0.0)),
            OnScreenKeyboards::ScaleAndReference => (Some(1.0 / 3.0), Some(0.0), None),
            OnScreenKeyboards::None => (None, None, None),
        };

    if let Some(reference_keyboard_location) = reference_keyboard_location {
        create_linear_keyboard(
            commands,
            meshes,
            materials,
            view_model,
            (
                &view_model.reference_scl,
                KbmRoot::from(Note::from_piano_key(kbm_root.ref_key)),
            ),
            |key| get_12edo_key_color(key + kbm_root.ref_key.midi_number()),
            reference_keyboard_location,
        );
    }

    if let Some(scale_keyboard_location) = scale_keyboard_location {
        create_linear_keyboard(
            commands,
            meshes,
            materials,
            view_model,
            (&state.scl, kbm_root),
            |key| {
                view_model.scale_keyboard_colors[usize::from(math::i32_rem_u(
                    key,
                    u16::try_from(view_model.scale_keyboard_colors.len()).unwrap(),
                ))]
            },
            scale_keyboard_location,
        );
    }

    if let Some(keyboard_location) = keyboard_location {
        create_isomorphic_keyboard(
            commands,
            meshes,
            materials,
            virtual_layout,
            view_model,
            kbm_root,
            |key| {
                view_model.scale_keyboard_colors[usize::from(math::i32_rem_u(
                    key,
                    u16::try_from(view_model.scale_keyboard_colors.len()).unwrap(),
                ))]
            },
            keyboard_location,
        );
    }
}

fn press_keys(
    state: ResMut<PianoEngineStateResource>,
    keyboards: Query<(&mut LinearKeyboard, &Children)>,
    mut keys: Query<&mut Transform>,
) {
    for (keyboard, children) in &keyboards {
        for (key_index, amount) in keyboard.pressed_keys(&state.0) {
            press_key(&mut keys.get_mut(children[key_index]).unwrap(), amount);
        }
    }
}

#[derive(Component)]
struct LinearKeyboard {
    scl: Scl,
    kbm_root: KbmRoot,
    key_range: RangeInclusive<i32>,
}

fn create_linear_keyboard(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    view_model: &ViewModel,
    tuning: (&Scl, KbmRoot),
    get_key_color: impl Fn(i32) -> KeyColor,
    vertical_position: f32,
) {
    let (key_range, grid_coords) = iterate_grid_coords(view_model, tuning, 1);

    let mut keyboard = commands.spawn((
        LinearKeyboard {
            scl: tuning.0.clone(),
            kbm_root: tuning.1,
            key_range,
        },
        SpatialBundle {
            transform: Transform::from_xyz(0.0, 0.0, SCENE_HEIGHT_3D * vertical_position),
            ..default()
        },
    ));
    let mesh = meshes.add(Cube::default().into());

    let (mut mid, mut right) = default();
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
                SCENE_HEIGHT_3D / 3.0 * KEYBOARD_VERT_FILL,
            );

            let mut transform = Transform::from_scale(key_box_size);
            transform.translation.x = key_center;
            reset_key(&mut transform);

            let key_color = map_key_color(get_key_color(drawn_key));

            keyboard.with_children(|commands| {
                let mut key = commands.spawn(MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: materials.add(key_material(key_color)),
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
        Quat::from_rotation_x((1.5 * amount as f32).to_radians()),
    );
}

#[derive(Component)]
struct IsomorphicKeyboard;

fn create_isomorphic_keyboard(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    virtual_layout: &VirtualKeyboardLayout,
    view_model: &ViewModel,
    kbm_root: KbmRoot,
    get_key_color: impl Fn(i32) -> KeyColor,
    vertical_position: f32,
) {
    const KEY_SIZE: f32 = 0.95;
    const KEY_HEIGHT: f32 = 0.75;
    const INCLINATION: f32 = 15.0;

    let primary_step = Vec2::new(1.0, 0.0); // Hexagonal east direction
    let secondary_step = Vec2::new(0.5, -0.5 * 3f32.sqrt()); // Hexagonal south-east direction

    let period_vector = f32::from(virtual_layout.num_primary_steps) * primary_step
        + f32::from(virtual_layout.num_secondary_steps) * secondary_step;

    let stride = 1.0
        / (view_model
            .pitch_range()
            .num_equal_steps_of_size(virtual_layout.period) as f32)
        / period_vector.length();

    let board_angle = period_vector.angle_between(Vec2::X);
    let board_rotation = Mat2::from_angle(board_angle);

    let primary_stride = stride * (board_rotation * primary_step);
    let secondary_stride = stride * (board_rotation * secondary_step);

    let inclination_factor = INCLINATION.to_radians().tan();
    let primary_stride_3d = Vec3::new(
        primary_stride.x,
        primary_stride.y * inclination_factor,
        -primary_stride.y,
    );
    let secondary_stride_3d = Vec3::new(
        secondary_stride.x,
        secondary_stride.y * inclination_factor,
        -secondary_stride.y,
    );

    let radius = KEY_SIZE * stride / 3f32.sqrt();
    let height = KEY_HEIGHT * stride / 3f32.sqrt();
    let key = Cylinder {
        radius,
        height,
        resolution: 6,
        segments: 1,
    };
    let mesh = meshes.add(key.into());
    let key_rotation = Quat::from_rotation_y(board_angle + 90f32.to_radians());

    let mut keyboard = commands.spawn((
        IsomorphicKeyboard,
        SpatialBundle {
            transform: Transform::from_xyz(0.0, 0.0, SCENE_HEIGHT_3D * vertical_position),
            ..default()
        },
    ));

    let offset = view_model.hor_world_coord(kbm_root.ref_pitch) as f32;
    let size = SCENE_HEIGHT_3D / 6.0 * KEYBOARD_VERT_FILL;

    let (p_range, s_range) = ortho_bounding_box_to_hex_bounding_box(
        primary_stride,
        secondary_stride,
        SCENE_LEFT - offset..SCENE_RIGHT - offset,
        -size..size,
    );

    for p in p_range {
        for s in s_range.clone() {
            let translation =
                hex_coord_to_ortho_coord(primary_stride_3d, secondary_stride_3d, p, s)
                    + offset * Vec3::X;

            if !is_in_ortho_bounding_box(
                SCENE_LEFT - radius..SCENE_RIGHT + radius,
                -size..size,
                translation,
            ) {
                continue;
            }

            let color = map_key_color(get_key_color(
                virtual_layout.keyboard.get_key(p, -s).midi_number(),
            ));

            let material = materials.add(key_material(color));

            keyboard.with_children(|commands| {
                commands.spawn(MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform: Transform::from_translation(translation).with_rotation(key_rotation),
                    ..default()
                });
            });
        }
    }
}

fn ortho_bounding_box_to_hex_bounding_box(
    primary_stride: Vec2,
    secondary_stride: Vec2,
    x_range: Range<f32>,
    y_range: Range<f32>,
) -> (RangeInclusive<i16>, RangeInclusive<i16>) {
    let ortho_corners = [
        Vec2::new(x_range.start, y_range.start),
        Vec2::new(x_range.start, y_range.end),
        Vec2::new(x_range.end, y_range.start),
        Vec2::new(x_range.end, y_range.end),
    ];

    let ortho_to_hex = Mat2::from_cols(primary_stride, secondary_stride).inverse();
    let hex_corners = ortho_corners.map(|corner| ortho_to_hex * corner);

    let [p1, p2, p3, p4] = hex_corners.map(|corner| corner.x);
    let [s1, s2, s3, s4] = hex_corners.map(|corner| corner.y);

    let p_min = p1.min(p2).min(p3).min(p4).floor() as i16;
    let p_max = p1.max(p2).max(p3).max(p4).ceil() as i16;
    let s_min = s1.min(s2).min(s3).min(s4).floor() as i16;
    let s_max = s1.max(s2).max(s3).max(s4).ceil() as i16;

    (p_min..=p_max, s_min..=s_max)
}

fn hex_coord_to_ortho_coord(primary_stride: Vec3, secondary_stride: Vec3, p: i16, s: i16) -> Vec3 {
    f32::from(p) * primary_stride + f32::from(s) * secondary_stride
}

fn is_in_ortho_bounding_box(x_range: Range<f32>, y_range: Range<f32>, translation: Vec3) -> bool {
    x_range.contains(&translation.x) && y_range.contains(&(translation.z - translation.y))
}

fn map_key_color(get_key_color: KeyColor) -> Color {
    match get_key_color {
        KeyColor::White => Color::WHITE,
        KeyColor::Black => Color::BLACK,
        KeyColor::Red => Color::MAROON,
        KeyColor::Green => Color::DARK_GREEN,
        KeyColor::Blue => Color::BLUE,
        KeyColor::Cyan => Color::TEAL,
        KeyColor::Magenta => Color::rgb(0.5, 0.0, 1.0),
        KeyColor::Yellow => Color::OLIVE,
    }
}

fn key_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.0,
        metallic: 1.5,
        ..default()
    }
}

#[derive(Component)]
struct GridLines;

fn create_grid_lines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    state: &PianoEngineState,
    view_model: &ViewModel,
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
    for (degree, pitch_coord) in iterate_grid_coords(view_model, tuning, 0).1 {
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
    view_model: &'a ViewModel,
    tuning: (&'a Scl, KbmRoot),
    padding: i32,
) -> (RangeInclusive<i32>, impl Iterator<Item = (i32, f32)> + 'a) {
    let range = tunable::range(tuning, view_model.viewport_left, view_model.viewport_right);
    let padded_range = range.start() - padding..=range.end() + padding;

    (
        range,
        padded_range.map(move |key_degree| {
            (
                key_degree,
                view_model.hor_world_coord(tuning.sorted_pitch_of(key_degree)) as f32,
            )
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
    view_model: &ViewModel,
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

    let octave_range = view_model.pitch_range().as_octaves();

    let mut freqs_hz = state
        .pressed_keys
        .values()
        .filter(|pressed_key| !pressed_key.shadow)
        .map(|pressed_key| pressed_key.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = freqs_hz.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_coord = view_model.hor_world_coord(*second) as f32;

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
                Ratio::between_pitches(*first, *second).nearest_fraction(view_model.odd_limit);

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
    const LINE_HEIGHT: f32 = SCENE_HEIGHT_2D / 36.0;

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
    view_model: Res<ViewModel>,
) {
    for info_update in info_updates.0.try_iter() {
        *info = info_update;
    }
    for mut hud_text in &mut hud_texts {
        hud_text.sections[0] = TextSection::new(
            create_hud_text(&state.0, &view_model, &info),
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
    view_settings: &ViewModel,
    info: &DynBackendInfo,
) -> String {
    let mut hud_text = String::new();

    writeln!(
        hud_text,
        "Scale: {scale}\n\
         Reference note [Alt+Left/Right]: {ref_note}\n\
         Scale offset [Left/Right]: {offset:+}\n\
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
        "Tuning mode [Alt+T]: {tuning_mode:?}\n\
         Legato [Alt+L]: {legato}\n\
         Effects [F1-F10]: {effects}\n\
         Recording [Space]: {recording}\n\
         On-screen keyboard [Alt+K]: {keyboard}\n\
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
        keyboard = match view_settings.on_screen_keyboards {
            OnScreenKeyboards::Isomorphic => "Isomorphic",
            OnScreenKeyboards::Scale => "Scale",
            OnScreenKeyboards::Reference => "Reference",
            OnScreenKeyboards::IsomorphicAndReference => "Isomorphic + Reference",
            OnScreenKeyboards::ScaleAndReference => "Scale + Reference",
            OnScreenKeyboards::None => "OFF",
        },
        from = view_settings.viewport_left.as_hz(),
        to = view_settings.viewport_right.as_hz(),
    )
    .unwrap();

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
             Program [Up/Down]: {program_number}",
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
