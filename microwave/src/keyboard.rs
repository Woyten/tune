use std::str::FromStr;

use bevy::prelude::*;

pub fn calc_hex_location(
    layout: KeyboardLayout,
    scan_code: u32,
    key_code: Option<KeyCode>,
) -> Option<(i8, i8)> {
    let physical_key = if cfg!(target_arch = "wasm32") {
        // We treat key codes (i.e. virtual keys) as physical keys since winit(wasm32) confounds them
        key_code
    } else {
        key_for_scancode(scan_code)
    };

    physical_key.and_then(|physical_key| hex_location_for_key(layout, physical_key))
}

fn key_for_scancode(scan_code: u32) -> Option<KeyCode> {
    Some(match scan_code {
        41 => KeyCode::Grave, // web: Backquote
        2 => KeyCode::Key1,
        3 => KeyCode::Key2,
        4 => KeyCode::Key3,
        5 => KeyCode::Key4,
        6 => KeyCode::Key5,
        7 => KeyCode::Key6,
        8 => KeyCode::Key7,
        9 => KeyCode::Key8,
        10 => KeyCode::Key9,
        11 => KeyCode::Key0,
        12 => KeyCode::Minus,
        13 => KeyCode::Equals,
        14 => KeyCode::Back, // web: Backspace
        // ---
        15 => KeyCode::Tab,
        16 => KeyCode::Q,
        17 => KeyCode::W,
        18 => KeyCode::E,
        19 => KeyCode::R,
        20 => KeyCode::T,
        21 => KeyCode::Y,
        22 => KeyCode::U,
        23 => KeyCode::I,
        24 => KeyCode::O,
        25 => KeyCode::P,
        26 => KeyCode::LBracket,
        27 => KeyCode::RBracket,
        28 => KeyCode::Return, // web: Enter
        // ---
        58 => KeyCode::Capital, // web: CapsLock - ignored by winit
        30 => KeyCode::A,
        31 => KeyCode::S,
        32 => KeyCode::D,
        33 => KeyCode::F,
        34 => KeyCode::G,
        35 => KeyCode::H,
        36 => KeyCode::J,
        37 => KeyCode::K,
        38 => KeyCode::L,
        39 => KeyCode::Semicolon,
        40 => KeyCode::Apostrophe, // web: Quote
        43 => KeyCode::Backslash,
        // ---
        42 => KeyCode::LShift,
        86 => KeyCode::Unlabeled, // web: IntlBackslash - ignored by winit
        44 => KeyCode::Z,
        45 => KeyCode::X,
        46 => KeyCode::C,
        47 => KeyCode::V,
        48 => KeyCode::B,
        49 => KeyCode::N,
        50 => KeyCode::M,
        51 => KeyCode::Comma,
        52 => KeyCode::Period,
        53 => KeyCode::Slash,
        54 => KeyCode::RShift,
        _ => return None,
    })
}

fn hex_location_for_key(layout: KeyboardLayout, physical_key: KeyCode) -> Option<(i8, i8)> {
    Some(match (physical_key, layout) {
        (KeyCode::Grave, _) => (-6, 1),
        (KeyCode::Key1, _) => (-5, 1),
        (KeyCode::Key2, _) => (-4, 1),
        (KeyCode::Key3, _) => (-3, 1),
        (KeyCode::Key4, _) => (-2, 1),
        (KeyCode::Key5, _) => (-1, 1),
        (KeyCode::Key6, _) => (0, 1),
        (KeyCode::Key7, _) => (1, 1),
        (KeyCode::Key8, _) => (2, 1),
        (KeyCode::Key9, _) => (3, 1),
        (KeyCode::Key0, _) => (4, 1),
        (KeyCode::Minus, _) => (5, 1),
        (KeyCode::Equals, _) => (6, 1),
        (KeyCode::Back, KeyboardLayout::Ansi)
        | (KeyCode::Backslash, KeyboardLayout::Variant)
        | (KeyCode::Back, KeyboardLayout::Iso) => (7, 1),
        (KeyCode::Back, KeyboardLayout::Variant) => (8, 1),
        // ---
        (KeyCode::Tab, _) => (-6, 0),
        (KeyCode::Q, _) => (-5, 0),
        (KeyCode::W, _) => (-4, 0),
        (KeyCode::E, _) => (-3, 0),
        (KeyCode::R, _) => (-2, 0),
        (KeyCode::T, _) => (-1, 0),
        (KeyCode::Y, _) => (0, 0),
        (KeyCode::U, _) => (1, 0),
        (KeyCode::I, _) => (2, 0),
        (KeyCode::O, _) => (3, 0),
        (KeyCode::P, _) => (4, 0),
        (KeyCode::LBracket, _) => (5, 0),
        (KeyCode::RBracket, _) => (6, 0),
        (KeyCode::Backslash, KeyboardLayout::Ansi) | (KeyCode::Return, KeyboardLayout::Iso) => {
            (7, 0)
        }
        // ---
        (KeyCode::Capital, _) => (-6, -1),
        (KeyCode::A, _) => (-5, -1),
        (KeyCode::S, _) => (-4, -1),
        (KeyCode::D, _) => (-3, -1),
        (KeyCode::F, _) => (-2, -1),
        (KeyCode::G, _) => (-1, -1),
        (KeyCode::H, _) => (0, -1),
        (KeyCode::J, _) => (1, -1),
        (KeyCode::K, _) => (2, -1),
        (KeyCode::L, _) => (3, -1),
        (KeyCode::Semicolon, _) => (4, -1),
        (KeyCode::Apostrophe, _) => (5, -1),
        (KeyCode::Return, KeyboardLayout::Ansi)
        | (KeyCode::Return, KeyboardLayout::Variant)
        | (KeyCode::Backslash, KeyboardLayout::Iso) => (6, -1),
        // ---
        (KeyCode::LShift, KeyboardLayout::Iso) => (-7, -2),
        (KeyCode::LShift, KeyboardLayout::Ansi)
        | (KeyCode::LShift, KeyboardLayout::Variant)
        | (KeyCode::Unlabeled, KeyboardLayout::Iso) => (-6, -2),
        (KeyCode::Z, _) => (-5, -2),
        (KeyCode::X, _) => (-4, -2),
        (KeyCode::C, _) => (-3, -2),
        (KeyCode::V, _) => (-2, -2),
        (KeyCode::B, _) => (-1, -2),
        (KeyCode::N, _) => (0, -2),
        (KeyCode::M, _) => (1, -2),
        (KeyCode::Comma, _) => (2, -2),
        (KeyCode::Period, _) => (3, -2),
        (KeyCode::Slash, _) => (4, -2),
        (KeyCode::RShift, _) => (5, -2),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
pub enum KeyboardLayout {
    Ansi,
    Variant,
    Iso,
}

impl FromStr for KeyboardLayout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const ANSI: &str = "ansi";
        const VAR: &str = "var";
        const ISO: &str = "iso";

        match s {
            ANSI => Ok(Self::Ansi),
            VAR => Ok(Self::Variant),
            ISO => Ok(Self::Iso),
            _ => Err(format!(
                "Invalid keyboard layout. Should be `{ANSI}`, `{VAR}` or `{ISO}`."
            )),
        }
    }
}
