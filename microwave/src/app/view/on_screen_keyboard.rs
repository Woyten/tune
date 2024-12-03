use std::{
    collections::HashMap,
    ops::{Range, RangeInclusive},
};

use bevy::{color::palettes::css, ecs::system::EntityCommands, prelude::*};
use tune::{
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
    tuning::Scale,
};

use crate::app::resources::{virtual_keyboard::VirtualKeyboardResource, MainViewResource};

#[derive(Component)]
pub struct OnScreenKeyboard {
    tuning: (Scl, KbmRoot),
    keys: HashMap<i32, Vec<OnScreenKey>>,
}

impl OnScreenKeyboard {
    pub fn get_keys(&self, pitch: Pitch) -> impl Iterator<Item = (&OnScreenKey, f64)> + '_ {
        self.get_interpolated_degrees(pitch)
            .into_iter()
            .flat_map(|(degree, amount)| {
                self.keys
                    .get(&degree)
                    .into_iter()
                    .flatten()
                    .map(move |key| (key, amount))
            })
    }

    fn get_interpolated_degrees(&self, pitch: Pitch) -> [(i32, f64); 2] {
        // Matching precise pitches is broken due to https://github.com/rust-lang/rust/issues/107904.
        let pitch = pitch * Ratio::from_float(0.999999);

        let approximation = self.tuning.find_by_pitch_sorted(pitch);
        let deviation_from_closest = approximation.deviation.as_octaves();

        let closest_degree = approximation.approx_value;
        let second_closest_degree = if deviation_from_closest < 0.0 {
            closest_degree - 1
        } else {
            closest_degree + 1
        };

        let second_closest_pitch = self.tuning.sorted_pitch_of(second_closest_degree);
        let deviation_from_second_closest =
            Ratio::between_pitches(pitch, second_closest_pitch).as_octaves();

        let interpolation = deviation_from_second_closest
            / (deviation_from_closest + deviation_from_second_closest);

        [
            (closest_degree, interpolation),
            (second_closest_degree, 1.0 - interpolation),
        ]
    }
}

pub struct OnScreenKey {
    pub entity: Entity,
    pub rotation_point: Vec3,
}

pub struct KeyboardCreator<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub main_view: &'a MainViewResource,
    pub height: f32,
    pub width: f32,
}

impl KeyboardCreator<'_, '_, '_> {
    pub fn create_linear(
        &mut self,
        tuning: (Scl, KbmRoot),
        get_key_color: impl Fn(i32) -> Srgba,
        vertical_position: f32,
    ) {
        const WIDTH_FACTOR: f32 = 0.9;
        const HEIGHT_FACTOR: f32 = 0.5;

        let mut keys = HashMap::<_, Vec<_>>::new();

        let key_geometry = self.meshes.add(Cuboid::default());

        let mut keyboard = self.commands.spawn(SpatialBundle {
            transform: Transform::from_xyz(0.0, 0.0, vertical_position),
            ..default()
        });

        let mut left;
        let (mut mid, mut right) = default();
        for (iterated_key, grid_coord) in super::iterate_grid_coords(self.main_view, &tuning) {
            (left, mid, right) = (mid, right, Some(grid_coord * self.width));

            if let (Some(left), Some(mid), Some(right)) = (left, mid, right) {
                let drawn_key = iterated_key - 1;

                let key_color = get_key_color(drawn_key);

                let key_center = (left + right) / 4.0 + mid / 2.0;
                let available_width = ((right - left) / 2.0).max(0.0);

                let key_scale = Vec3::new(
                    available_width * WIDTH_FACTOR,
                    available_width * HEIGHT_FACTOR,
                    self.height,
                );

                let mut transform = Transform::from_scale(key_scale);
                transform.translation.x = key_center;
                transform.translation.y = -transform.scale.y / 2.0;

                keyboard.with_children(|commands| {
                    let mut key = create_key(
                        commands,
                        &key_geometry,
                        self.materials,
                        key_color,
                        transform,
                    );

                    if drawn_key == 0 {
                        add_key_marker(
                            &mut key,
                            &key_geometry,
                            self.materials,
                            key_scale,
                            available_width,
                        );
                    }

                    keys.entry(drawn_key).or_default().push(OnScreenKey {
                        entity: key.id(),
                        rotation_point: Vec3::NEG_Z * transform.scale.z,
                    });
                });
            }
        }

        keyboard.insert(OnScreenKeyboard { tuning, keys });
    }

    pub fn create_isomorphic(
        &mut self,
        virtual_keyboard: &VirtualKeyboardResource,
        tuning: (Scl, KbmRoot),
        get_key_color: impl Fn(i32) -> Srgba,
        vertical_position: f32,
    ) {
        const RADIUS_FACTOR: f32 = 0.95;
        const HEIGHT_FACTOR: f32 = 0.5;
        const ROTATION_POINT_FACTOR: f32 = 10.0;

        let (num_primary_steps, num_secondary_steps) = virtual_keyboard.layout_step_counts();
        let (primary_step, secondary_step, ..) = virtual_keyboard.layout_step_sizes();
        let geom_primary_step = Vec2::new(1.0, 0.0); // Hexagonal east direction
        let geom_secondary_step = Vec2::new(0.5, -0.5 * 3f32.sqrt()); // Hexagonal south-east direction

        let period = virtual_keyboard
            .avg_step_size
            .repeated(num_primary_steps * primary_step + num_secondary_steps * secondary_step);
        let geom_period = num_primary_steps as f32 * geom_primary_step
            + num_secondary_steps as f32 * geom_secondary_step;

        let board_angle = geom_period.angle_between(Vec2::X);
        let board_rotation = Mat2::from_angle(board_angle);

        let key_stride = period
            .divided_into_equal_steps(geom_period.length())
            .num_equal_steps_of_size(self.main_view.pitch_range()) as f32;

        let primary_stride_2d = key_stride * (board_rotation * geom_primary_step);
        let secondary_stride_2d = key_stride * (board_rotation * geom_secondary_step);

        let (x_range, y_range) = self.get_bounding_box();
        let offset = self.main_view.hor_world_coord(tuning.1.ref_pitch) as f32;

        let (p_range, s_range) = ortho_bounding_box_to_hex_bounding_box(
            primary_stride_2d,
            secondary_stride_2d,
            x_range.start - offset..x_range.end - offset,
            y_range.clone(),
        );

        let slope = virtual_keyboard.inclination().to_radians().tan();
        let primary_stride = Vec3::new(
            primary_stride_2d.x,
            primary_stride_2d.y * slope,
            -primary_stride_2d.y,
        );
        let secondary_stride = Vec3::new(
            secondary_stride_2d.x,
            secondary_stride_2d.y * slope,
            -secondary_stride_2d.y,
        );

        let key_rotation = Quat::from_rotation_y(board_angle + 90f32.to_radians());

        let key_scale = Vec3::new(
            key_stride * RADIUS_FACTOR,
            key_stride * HEIGHT_FACTOR,
            key_stride * RADIUS_FACTOR,
        );

        let mut keys = HashMap::<_, Vec<_>>::new();

        let key_geometry = self.meshes.add(
            Cylinder {
                radius: 1.0 / 3f32.sqrt(),
                ..default()
            }
            .mesh()
            .resolution(6),
        );

        let mut keyboard = self.commands.spawn(SpatialBundle {
            transform: Transform::from_xyz(0.0, 0.0, vertical_position),
            ..default()
        });

        for p in p_range {
            for s in s_range.clone() {
                let translation = hex_coord_to_ortho_coord(primary_stride, secondary_stride, p, s)
                    + offset * Vec3::X;

                let should_draw = is_in_ortho_bounding_box(
                    x_range.start - key_stride..x_range.end + key_stride,
                    y_range.clone(),
                    translation,
                );

                if !should_draw {
                    continue;
                }

                let key_degree = virtual_keyboard.get_key(p, s) - tuning.1.root_offset;
                let key_color = get_key_color(key_degree);

                let transform = Transform::from_translation(translation)
                    .with_scale(key_scale)
                    .with_rotation(key_rotation);

                keyboard.with_children(|commands| {
                    let mut key = create_key(
                        commands,
                        &key_geometry,
                        self.materials,
                        key_color,
                        transform,
                    );

                    if key_degree == 0 {
                        add_key_marker(
                            &mut key,
                            &key_geometry,
                            self.materials,
                            key_scale,
                            key_stride,
                        );
                    }

                    keys.entry(key_degree).or_default().push(OnScreenKey {
                        entity: key.id(),
                        rotation_point: Vec3::NEG_Z * key_stride * ROTATION_POINT_FACTOR,
                    });
                });
            }
        }

        keyboard.insert(OnScreenKeyboard { tuning, keys });
    }

    fn get_bounding_box(&self) -> (Range<f32>, Range<f32>) {
        (
            -self.width / 2.0..self.width / 2.0,
            -self.height / 2.0..self.height / 2.0,
        )
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

fn create_key<'a>(
    commands: &'a mut ChildBuilder,
    geometry: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    color: Srgba,
    transform: Transform,
) -> EntityCommands<'a> {
    let material = get_mesh_material(color);
    create_mesh(commands, geometry, materials, material, transform)
}

fn add_key_marker(
    parent_key: &mut EntityCommands,
    geometry: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent_key_scale: Vec3,
    available_width: f32,
) {
    let material = StandardMaterial {
        alpha_mode: AlphaMode::Blend,
        ..get_mesh_material(css::RED.with_alpha(0.5))
    };

    let available_margin = available_width - parent_key_scale.x;
    let wanted_marker_scale = parent_key_scale + available_margin;
    let marker_scale_wrt_parent = wanted_marker_scale / parent_key_scale;
    let transform = Transform::from_scale(marker_scale_wrt_parent);

    parent_key.with_children(|commands| {
        create_mesh(commands, geometry, materials, material, transform);
    });
}

fn get_mesh_material(color: Srgba) -> StandardMaterial {
    StandardMaterial {
        base_color: color.into(),
        perceptual_roughness: 0.0,
        metallic: 1.5,
        ..default()
    }
}

fn create_mesh<'a>(
    commands: &'a mut ChildBuilder,
    geometry: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    material: StandardMaterial,
    transform: Transform,
) -> EntityCommands<'a> {
    commands.spawn(MaterialMeshBundle {
        mesh: geometry.clone(),
        material: materials.add(material),
        transform,
        ..default()
    })
}
