use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;

pub fn generalized_cylinder(num_corners: u16, rings: &[(f32, f32)]) -> Mesh {
    let mut positions = rings
        .iter()
        .flat_map(|&(y, radius)| {
            (0..num_corners).map(move |i| {
                Vec3::new(radius, y, 0.0).rotate_y(f32::from(i) * TAU / f32::from(num_corners))
            })
        })
        .collect::<Vec<_>>();

    positions.push(Vec3::new(0.0, rings.first().unwrap().0, 0.0));
    let bottom_center_idx = (positions.len() - 1) as u32;
    positions.push(Vec3::new(0.0, rings.last().unwrap().0, 0.0));
    let top_center_idx = (positions.len() - 1) as u32;

    let last_ring_index = rings.len() as u32 - 1;

    let num_corners = u32::from(num_corners);
    let mut indices = Vec::new();
    for i in 0..num_corners {
        let j = (i + 1) % num_corners;

        for r1 in 0..last_ring_index {
            let r2 = r1 + 1;
            let a = r1 * num_corners + i;
            let b = r1 * num_corners + j;
            let c = r2 * num_corners + j;
            let d = r2 * num_corners + i;
            indices.extend_from_slice(&[a, b, c, c, d, a]);
        }

        indices.extend_from_slice(&[bottom_center_idx, j, i]);
        indices.extend_from_slice(&[
            top_center_idx,
            last_ring_index * num_corners + i,
            last_ring_index * num_corners + j,
        ]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh
}
