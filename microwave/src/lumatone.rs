use std::time::Duration;

use async_std::task;
use bevy::render::color::Color;
use flume::Sender;
use rand::{rngs::SmallRng, seq::SliceRandom, SeedableRng};
use tune::math;
use tune_cli::shared::midi::{self, MidiResult};

use crate::{app::VirtualKeyboardResource, portable};

// 16 channels à 128 notes
pub const RANGE_RADIUS: i32 = 1024;

pub fn connect_lumatone(fuzzy_port_name: &str) -> MidiResult<Sender<LumatoneLayout>> {
    let (_, mut connection) = midi::connect_to_out_device("microwave", fuzzy_port_name)?;

    let (send, recv) = flume::unbounded::<LumatoneLayout>();

    portable::spawn_task(async move {
        let mut rng = SmallRng::from_entropy();

        while let Ok(LumatoneLayout(mut layout)) = recv.recv_async().await {
            layout.shuffle(&mut rng);

            for ((board, key), (channel, note, color)) in layout {
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
                        board + 1,
                        0x00,
                        key,
                        note,
                        channel,
                        0x01,
                        0xf7,
                    ])
                    .unwrap();

                // Set color

                let (r, g, b) = (
                    (color.r() * 255.0) as u8,
                    (color.g() * 255.0) as u8,
                    (color.b() * 255.0) as u8,
                );

                connection
                    .send(&[
                        0xf0,
                        0x00,
                        0x21,
                        0x50,
                        board + 1,
                        0x01,
                        key,
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

pub struct LumatoneLayout(Vec<LumatoneKeyConfig>);
type LumatoneKeyConfig = ((u8, u8), (u8, u8, Color));

impl LumatoneLayout {
    pub fn from_fn(color_fn: impl FnMut(i16, i16) -> (u8, u8, Color)) -> Self {
        Self(vec_of_keys(color_fn))
    }

    pub fn from_virtual_keyboard(virtual_keyboard: &VirtualKeyboardResource) -> LumatoneLayout {
        Self::from_fn(|p, s| {
            let degree = virtual_keyboard.get_key(p, s);
            let colors = &virtual_keyboard.colors();

            let channel = u8::try_from(degree.div_euclid(128) + 8);
            let key = u8::try_from(degree.rem_euclid(128));

            let color = if channel.is_ok() && key.is_ok() {
                colors[usize::from(math::i32_rem_u(
                    degree,
                    u16::try_from(colors.len()).unwrap(),
                ))]
            } else {
                Color::BLACK
            };
            (channel.unwrap_or_default(), key.unwrap_or_default(), color)
        })
    }
}

fn vec_of_keys<T>(mut value_fn: impl FnMut(i16, i16) -> T) -> Vec<((u8, u8), T)> {
    let coord_of_d = KEY_COORDS[20];

    let mut result = Vec::new();

    for board_index in 0..5 {
        for (key_coord, key_index) in KEY_COORDS.iter().zip(0..) {
            result.push((
                (board_index, key_index),
                value_fn(
                    5 * (i16::from(board_index) - 2) + i16::from(key_coord.0)
                        - i16::from(coord_of_d.0),
                    2 * (i16::from(board_index) - 2) + i16::from(key_coord.1)
                        - i16::from(coord_of_d.1),
                ),
            ))
        }
    }

    result
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
