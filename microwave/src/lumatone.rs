use std::path::Path;
use std::time::Duration;

use async_std::task;
use bevy::color::palettes::css;
use bevy::prelude::*;
use flume::Sender;
use image::GenericImageView;
use image::imageops;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use tune::math;
use tune_cli::shared::midi;
use tune_cli::shared::midi::MidiResult;

use crate::lumatone::hexmath::HexGeometry;
use crate::portable;
use crate::tuning_layout::TuningLayout;

pub fn connect_lumatone(fuzzy_port_name: &str) -> MidiResult<Sender<LumatoneLayout>> {
    let (_, mut connection) = midi::connect_to_out_device("microwave", fuzzy_port_name)?;

    let (send, recv) = flume::unbounded::<LumatoneLayout>();

    portable::spawn_task(async move {
        let mut rng = SmallRng::seed_from_u64(0);

        while let Ok(LumatoneLayout(mut layout)) = recv.recv_async().await {
            layout.shuffle(&mut rng);

            for (key, color) in layout {
                // Abort materialization of current layout if new layouts are in the queue.
                if !recv.is_empty() {
                    continue;
                }

                // Set channel and note
                connection
                    .send(&[
                        0xf0,
                        0x00,
                        0x21,
                        0x50,
                        key.board_index + 1,
                        0x00,
                        key.key_index,
                        key.key_index,
                        key.board_index,
                        0x01,
                        0xf7,
                    ])
                    .unwrap();

                task::sleep(Duration::from_millis(15)).await;

                // Set color
                let (r, g, b) = (
                    (color.red * 255.0) as u8,
                    (color.green * 255.0) as u8,
                    (color.blue * 255.0) as u8,
                );

                connection
                    .send(&[
                        0xf0,
                        0x00,
                        0x21,
                        0x50,
                        key.board_index + 1,
                        0x01,
                        key.key_index,
                        r >> 4,
                        r & 0b1111,
                        g >> 4,
                        g & 0b1111,
                        b >> 4,
                        b & 0b1111,
                        0xf7,
                    ])
                    .unwrap();

                task::sleep(Duration::from_millis(15)).await;
            }
        }
    });

    Ok(send)
}

pub struct LumatoneImageColors {
    img: image::DynamicImage,
    img_w: u32,
    img_h: u32,
    geometry: HexGeometry,
    min_x: f32,
    width: f32,
    eff_max_y: f32,
}

impl LumatoneImageColors {
    pub fn load(path: &Path) -> image::ImageResult<Self> {
        let img = image::open(path)?;
        let (img_w, img_h) = img.dimensions();

        let geometry = HexGeometry::get(5, 2);
        let (min_x, _max_x, min_y, _max_y) =
            geometry.ortho_bounding_box(LumatoneKey::iter_all().map(|key| key.isomorphic_coord()));

        let width = _max_x - min_x;
        let center_y = (min_y + _max_y) / 2.0;
        let eff_max_y = center_y + width / 2.0;

        Ok(Self {
            img,
            img_w,
            img_h,
            geometry,
            min_x,
            width,
            eff_max_y,
        })
    }

    pub fn color_at(&self, p: i16, s: i16) -> Srgba {
        let ortho = self.geometry.ortho_coord(p, s);

        let px_x = (ortho.x - self.min_x) / self.width * (self.img_w - 1) as f32;
        let px_y = (self.eff_max_y - ortho.y) / self.width * (self.img_h - 1) as f32;

        match imageops::interpolate_bilinear(&self.img, px_x, px_y) {
            Some(pixel) => {
                let [r, g, b, _a] = pixel.0;
                Srgba::new(
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    1.0,
                )
            }
            None => css::BLACK,
        }
    }
}

pub struct LumatoneLayout(Vec<(LumatoneKey, Srgba)>);

impl LumatoneLayout {
    pub fn from_fn(mut color_fn: impl FnMut(&LumatoneKey) -> Srgba) -> Self {
        Self(
            LumatoneKey::iter_all()
                .map(|key| {
                    let color = color_fn(&key);
                    (key, color)
                })
                .collect(),
        )
    }

    pub fn from_tuning_layout(tuning_layout: &TuningLayout) -> Self {
        Self::from_fn(|key| {
            let (p, s) = key.isomorphic_coord();
            let degree = tuning_layout.get_key(p, s);
            let colors = &tuning_layout.colors();

            colors[usize::from(math::i32_rem_u(
                degree,
                u16::try_from(colors.len()).unwrap(),
            ))]
        })
    }

    pub fn with_image_colors(self, path: &Path) -> image::ImageResult<Self> {
        let image_colors = LumatoneImageColors::load(path)?;

        let LumatoneLayout(mut layout) = self;

        for (key, color) in &mut layout {
            let (p, s) = key.isomorphic_coord();
            *color = image_colors.color_at(p, s);
        }

        Ok(LumatoneLayout(layout))
    }
}

pub struct LumatoneKey {
    pub board_index: u8,
    pub key_index: u8,
}

impl LumatoneKey {
    pub fn iter_all() -> impl Iterator<Item = Self> {
        (0..5).flat_map(move |board_index| {
            (0..u8::try_from(KEY_COORDS.len()).unwrap()).map(move |key_index| LumatoneKey {
                board_index,
                key_index,
            })
        })
    }

    pub fn isomorphic_coord(&self) -> (i16, i16) {
        /// For symmetry reasons, middle D is used as the origin of the isomorphic layout.
        const ORIGIN: (u8, u8) = KEY_COORDS[20];

        let (x, y) = KEY_COORDS[usize::from(self.key_index) % KEY_COORDS.len()];
        (
            5 * (i16::from(self.board_index) - 2) + i16::from(x) - i16::from(ORIGIN.0),
            2 * (i16::from(self.board_index) - 2) + i16::from(y) - i16::from(ORIGIN.1),
        )
    }
}

const KEY_COORDS: [(u8, u8); 56] = [
    (4, 0),
    (5, 0),
    // --
    (4, 1),
    (5, 1),
    (6, 1),
    (7, 1),
    (8, 1),
    // --
    (3, 2),
    (4, 2),
    (5, 2),
    (6, 2),
    (7, 2),
    (8, 2),
    // --
    (3, 3),
    (4, 3),
    (5, 3),
    (6, 3),
    (7, 3),
    (8, 3),
    // --
    (2, 4), // C
    (3, 4), // D
    (4, 4), // E
    (5, 4), // F#
    (6, 4), // G#
    (7, 4), // A#
    // --
    (2, 5), // Db
    (3, 5), // Eb
    (4, 5), // F
    (5, 5), // G
    (6, 5), // A
    (7, 5), // B
    // --
    (1, 6),
    (2, 6),
    (3, 6),
    (4, 6),
    (5, 6),
    (6, 6),
    // --
    (1, 7),
    (2, 7),
    (3, 7),
    (4, 7),
    (5, 7),
    (6, 7),
    // --
    (0, 8),
    (1, 8),
    (2, 8),
    (3, 8),
    (4, 8),
    (5, 8),
    // --
    (1, 9),
    (2, 9),
    (3, 9),
    (4, 9),
    (5, 9),
    // --
    (3, 10),
    (4, 10),
];

pub mod hexmath {
    use bevy::math::Mat2;
    use bevy::math::Vec2;

    pub struct HexGeometry {
        pub primary: Vec2,
        pub secondary: Vec2,
        pub period_length: f32,
        pub angle: f32,
    }

    impl HexGeometry {
        pub fn get(num_primary_steps: i32, num_secondary_steps: i32) -> Self {
            let geom_primary = Vec2::new(1.0, 0.0);
            let geom_secondary = Vec2::new(0.5, -0.5 * 3f32.sqrt());

            let geom_period = num_primary_steps as f32 * geom_primary
                + num_secondary_steps as f32 * geom_secondary;
            let board_angle = geom_period.angle_to(Vec2::X);
            let board_rotation = Mat2::from_angle(board_angle);

            Self {
                primary: board_rotation * geom_primary,
                secondary: board_rotation * geom_secondary,
                period_length: geom_period.length(),
                angle: board_angle,
            }
        }

        pub fn ortho_bounding_box(
            &self,
            hex_coords: impl IntoIterator<Item = (i16, i16)>,
        ) -> (f32, f32, f32, f32) {
            hex_coords.into_iter().fold(
                (
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                ),
                |(min_x, max_x, min_y, max_y), (p, s)| {
                    let ortho = self.ortho_coord(p, s);
                    (
                        min_x.min(ortho.x),
                        max_x.max(ortho.x),
                        min_y.min(ortho.y),
                        max_y.max(ortho.y),
                    )
                },
            )
        }

        pub fn ortho_coord(&self, p: i16, s: i16) -> Vec2 {
            f32::from(p) * self.primary + f32::from(s) * self.secondary
        }
    }
}
