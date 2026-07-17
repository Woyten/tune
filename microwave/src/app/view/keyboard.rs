use std::collections::HashMap;
use std::f32::consts::*;
use std::ops::Range;
use std::ops::RangeInclusive;

use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::css;
use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use tune::pitch::Pitch;
use tune::pitch::Ratio;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::tuning::Scale;
use tune::tuning::Tuning;

use crate::app::shapes;
use crate::app::state::Tilt;
use crate::app::state::ViewState;
use crate::tuning_layout::TuningLayout;

#[derive(Component)]
pub struct OnScreenKeyboard {
    tuning: (Scl, KbmRoot),
    keys: HashMap<i32, Vec<OnScreenKey>>,
}

impl OnScreenKeyboard {
    pub fn get_all_keys(&mut self) -> impl Iterator<Item = &mut OnScreenKey> {
        self.keys.values_mut().flatten()
    }

    pub fn get_keys_for_pitch(
        &self,
        pitch: Pitch,
    ) -> impl Iterator<Item = (&OnScreenKey, f32)> + '_ {
        self.get_interpolated_degrees(pitch)
            .into_iter()
            .flat_map(|(degree, amount)| {
                self.keys
                    .get(&degree)
                    .into_iter()
                    .flatten()
                    .map(move |key| (key, amount as f32))
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
    pub transform: Transform,
    pub pivot: Vec3,
    pub press_amount: f32,
}

pub struct KeyboardCreator<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub view_state: &'a ViewState,
    pub depth: f32,
}

impl KeyboardCreator<'_, '_, '_> {
    pub fn create_linear(
        &mut self,
        tuning_layout: &TuningLayout,
        marker_degree: i32,
        render_layers: &RenderLayers,
    ) {
        const WIDTH_FACTOR: f32 = 0.9;
        const HEIGHT: f32 = 0.03; // 1.5° ≈ 0.026. Choose 0.03 s.t. the entire key can be seen, even when pressed.

        let tuning = (tuning_layout.scl.clone(), tuning_layout.kbm.root);

        let mut keys = HashMap::<_, Vec<_>>::new();

        let square_radius = FRAC_1_SQRT_2;
        let key_geometry = self.meshes.add({
            let mut mesh = shapes::generalized_cylinder(
                4,
                &[
                    (-0.5, square_radius * 0.98),
                    (-0.49, square_radius),
                    (0.49, square_radius),
                    (0.5, square_radius * 0.98),
                ],
            )
            .rotated_by(Quat::from_rotation_y(FRAC_PI_4))
            .rotated_by(Quat::from_rotation_x(FRAC_PI_2));
            mesh.duplicate_vertices();
            mesh.with_computed_flat_normals()
        });

        let mut keyboard = self.commands.spawn((
            Transform::default(),
            Visibility::default(),
            render_layers.clone(),
        ));

        let mut left;
        let (mut mid, mut right) = default();
        for (iterated_degree, grid_coord) in self.view_state.world_coords_of_tuning(&tuning) {
            (left, mid, right) = (mid, right, Some(grid_coord));

            if let (Some(left), Some(mid), Some(right)) = (left, mid, right) {
                let scale_degree = iterated_degree - 1;
                let key_color = tuning_layout.key_color(scale_degree);

                let key_center = (left + right) / 4.0 + mid / 2.0;
                let key_width = (right - left) / 2.0;

                let key_scale = Vec3::new(key_width * WIDTH_FACTOR, HEIGHT, self.depth);

                let transform =
                    Transform::from_scale(key_scale).with_translation(key_center * Vec3::X);

                keyboard.with_children(|commands| {
                    let entity = create_key(
                        commands,
                        &key_geometry,
                        self.materials,
                        key_color,
                        render_layers,
                        transform,
                        (scale_degree == marker_degree).then_some(key_width),
                    );

                    keys.entry(scale_degree).or_default().push(OnScreenKey {
                        entity,
                        transform,
                        pivot: self.depth * Vec3::NEG_Z,
                        press_amount: 0.0,
                    });
                });
            }
        }

        keyboard.insert(OnScreenKeyboard { tuning, keys });
    }

    pub fn create_isomorphic(
        &mut self,
        tuning_layout: &TuningLayout,
        marker_degree: i32,
        render_layers: &RenderLayers,
    ) {
        const RADIUS_FACTOR: f32 = 0.95;
        const HEIGHT_FACTOR: f32 = 0.5;
        const PIVOT_FACTOR: f32 = 10.0;

        let tuning = (tuning_layout.scl.clone(), tuning_layout.kbm.root);

        let (num_primary_steps, num_secondary_steps) = match self.view_state.tilt.curr_option() {
            Tilt::None => (1, 0),
            Tilt::Automatic => tuning_layout.layout_step_counts(),
            Tilt::Lumatone => (5, 2),
        };
        let (primary_step, secondary_step, _) = tuning_layout.layout_step_sizes();
        let geom_primary_step = Vec2::new(1.0, 0.0); // Hexagonal east direction
        let geom_secondary_step = Vec2::new(0.5, -0.5 * 3f32.sqrt()); // Hexagonal south-east direction

        let period = tuning_layout
            .avg_step_size()
            .repeated(num_primary_steps * primary_step + num_secondary_steps * secondary_step);
        let geom_period = num_primary_steps as f32 * geom_primary_step
            + num_secondary_steps as f32 * geom_secondary_step;

        let board_angle = geom_period.angle_to(Vec2::X);
        let board_rotation = Mat2::from_angle(board_angle);

        let key_stride = period
            .divided_into_equal_steps(geom_period.length())
            .num_equal_steps_of_size(self.view_state.pitch_range()) as f32;

        let primary_stride_2d = key_stride * (board_rotation * geom_primary_step);
        let secondary_stride_2d = key_stride * (board_rotation * geom_secondary_step);

        let x_range = -0.5 - key_stride..0.5 + key_stride;
        let y_range = -self.depth / 2.0 - key_stride..self.depth / 2.0 + key_stride;
        let offset = self.view_state.world_coord_of_pitch(tuning.pitch_of(0));

        let (p_range, s_range) = ortho_bounding_box_to_hex_bounding_box(
            primary_stride_2d,
            secondary_stride_2d,
            x_range.start - offset..x_range.end - offset,
            y_range.clone(),
        );

        let slope = self
            .view_state
            .inclination
            .curr_option()
            .degrees()
            .to_radians()
            .tan();
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

        let mut keys = HashMap::<_, Vec<_>>::new();

        let hex_radius = 1.0 / 3f32.sqrt();
        let key_geometry = self.meshes.add(
            shapes::generalized_cylinder(
                6,
                &[
                    (-0.5, hex_radius * 0.925),
                    (-0.45, hex_radius),
                    (0.45, hex_radius),
                    (0.5, hex_radius * 0.925),
                ],
            )
            .scaled_by(key_stride * Vec3::new(RADIUS_FACTOR, HEIGHT_FACTOR, RADIUS_FACTOR))
            .rotated_by(key_rotation)
            .with_computed_area_weighted_normals(),
        );

        let mut keyboard = self.commands.spawn((
            Transform::default(),
            Visibility::default(),
            render_layers.clone(),
        ));

        for p in p_range {
            for s in s_range.clone() {
                let translation = hex_coord_to_ortho_coord(primary_stride, secondary_stride, p, s)
                    + offset * Vec3::X;

                let should_draw =
                    is_in_ortho_bounding_box(x_range.clone(), y_range.clone(), translation);

                if !should_draw {
                    continue;
                }

                let scale_degree = tuning_layout.get_degree(p, s);
                let key_color = tuning_layout.key_color(scale_degree);

                let transform = Transform::from_translation(translation);

                keyboard.with_children(|commands| {
                    let entity = create_key(
                        commands,
                        &key_geometry,
                        self.materials,
                        key_color,
                        render_layers,
                        transform,
                        (scale_degree == marker_degree).then_some(1.0 / RADIUS_FACTOR),
                    );

                    keys.entry(scale_degree).or_default().push(OnScreenKey {
                        entity,
                        transform,
                        pivot: Vec3::NEG_Z * key_stride * PIVOT_FACTOR,
                        press_amount: 0.0,
                    });
                });
            }
        }

        keyboard.insert(OnScreenKeyboard { tuning, keys });
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

fn create_key(
    commands: &mut RelatedSpawnerCommands<ChildOf>,
    geometry: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    color: Srgba,
    render_layers: &RenderLayers,
    transform: Transform,
    marker_width: Option<f32>,
) -> Entity {
    // Values of roughness and reflectance result in a slight reflection that does not overpower the key color but provides a sense of depth.
    // The illumanated nature of the Lumatone is simulated by setting the emissive color in the keyboard update system.
    let mut material = StandardMaterial {
        base_color: color.into(),
        emissive: (color * 0.5).into(),
        perceptual_roughness: 0.4, // Spread of specular reflection
        metallic: 0.75,            // Metallic proportion, suppresses diffuse reflection
        reflectance: 0.0,          // Non-metallic reflectance, contains diffuse reflection
        ..default()
    };

    let mut key = create_mesh(
        commands,
        geometry,
        materials,
        material.clone(),
        render_layers,
        transform,
    );

    if let Some(marker_width) = marker_width {
        material.base_color = css::RED.with_alpha(0.5).into();
        material.emissive = css::RED.into();
        material.alpha_mode = AlphaMode::Blend;

        let available_margin = marker_width - transform.scale.x;
        let wanted_marker_scale = transform.scale + available_margin;
        let marker_scale_wrt_parent = wanted_marker_scale / transform.scale;
        let transform = Transform::from_scale(marker_scale_wrt_parent);

        key.with_children(|commands| {
            create_mesh(
                commands,
                geometry,
                materials,
                material,
                render_layers,
                transform,
            );
        });
    }

    key.id()
}

fn create_mesh<'a>(
    commands: &'a mut RelatedSpawnerCommands<ChildOf>,
    geometry: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    material: StandardMaterial,
    render_layers: &RenderLayers,
    transform: Transform,
) -> EntityCommands<'a> {
    commands.spawn((
        Mesh3d(geometry.clone()),
        MeshMaterial3d(materials.add(material)),
        transform,
        render_layers.clone(),
    ))
}
