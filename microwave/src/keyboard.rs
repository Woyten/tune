use std::str::FromStr;

use nannou::prelude::Key;

pub fn calc_hex_location(
    layout: KeyboardLayout,
    scancode: u32,
    virtual_key: Option<Key>,
) -> Option<(i8, i8)> {
    let physical_key = if cfg!(target_arch = "wasm32") {
        // We treat virtual keys as physical keys since winit(wasm32) confounds scancodes and virtual keycodes
        virtual_key
    } else {
        key_for_scancode(scancode)
    };

    physical_key.and_then(|key| hex_location_for_key(layout, key))
}

fn key_for_scancode(keycode: u32) -> Option<Key> {
    Some(match keycode {
        41 => Key::Grave, // web: Backquote
        2 => Key::Key1,
        3 => Key::Key2,
        4 => Key::Key3,
        5 => Key::Key4,
        6 => Key::Key5,
        7 => Key::Key6,
        8 => Key::Key7,
        9 => Key::Key8,
        10 => Key::Key9,
        11 => Key::Key0,
        12 => Key::Minus,
        13 => Key::Equals,
        14 => Key::Back, // web: Backspace
        // ---
        15 => Key::Tab,
        16 => Key::Q,
        17 => Key::W,
        18 => Key::E,
        19 => Key::R,
        20 => Key::T,
        21 => Key::Y,
        22 => Key::U,
        23 => Key::I,
        24 => Key::O,
        25 => Key::P,
        26 => Key::LBracket,
        27 => Key::RBracket,
        28 => Key::Return, // web: Enter
        // ---
        58 => Key::Capital, // web: CapsLock - ignored by winit
        30 => Key::A,
        31 => Key::S,
        32 => Key::D,
        33 => Key::F,
        34 => Key::G,
        35 => Key::H,
        36 => Key::J,
        37 => Key::K,
        38 => Key::L,
        39 => Key::Semicolon,
        40 => Key::Apostrophe, // web: Quote
        43 => Key::Backslash,
        // ---
        42 => Key::LShift,
        86 => Key::Unlabeled, // web: IntlBackslash - ignored by winit
        44 => Key::Z,
        45 => Key::X,
        46 => Key::C,
        47 => Key::V,
        48 => Key::B,
        49 => Key::N,
        50 => Key::M,
        51 => Key::Comma,
        52 => Key::Period,
        53 => Key::Slash,
        54 => Key::RShift,
        _ => return None,
    })
}

fn hex_location_for_key(layout: KeyboardLayout, physical_key: Key) -> Option<(i8, i8)> {
    Some(match (physical_key, layout) {
        (Key::Grave, _) => (-6, 1),
        (Key::Key1, _) => (-5, 1),
        (Key::Key2, _) => (-4, 1),
        (Key::Key3, _) => (-3, 1),
        (Key::Key4, _) => (-2, 1),
        (Key::Key5, _) => (-1, 1),
        (Key::Key6, _) => (0, 1),
        (Key::Key7, _) => (1, 1),
        (Key::Key8, _) => (2, 1),
        (Key::Key9, _) => (3, 1),
        (Key::Key0, _) => (4, 1),
        (Key::Minus, _) => (5, 1),
        (Key::Equals, _) => (6, 1),
        (Key::Back, KeyboardLayout::Ansi)
        | (Key::Backslash, KeyboardLayout::Variant)
        | (Key::Back, KeyboardLayout::Iso) => (7, 1),
        (Key::Back, KeyboardLayout::Variant) => (8, 1),
        // ---
        (Key::Tab, _) => (-6, 0),
        (Key::Q, _) => (-5, 0),
        (Key::W, _) => (-4, 0),
        (Key::E, _) => (-3, 0),
        (Key::R, _) => (-2, 0),
        (Key::T, _) => (-1, 0),
        (Key::Y, _) => (0, 0),
        (Key::U, _) => (1, 0),
        (Key::I, _) => (2, 0),
        (Key::O, _) => (3, 0),
        (Key::P, _) => (4, 0),
        (Key::LBracket, _) => (5, 0),
        (Key::RBracket, _) => (6, 0),
        (Key::Backslash, KeyboardLayout::Ansi) | (Key::Return, KeyboardLayout::Iso) => (7, 0),
        // ---
        (Key::Capital, _) => (-6, -1),
        (Key::A, _) => (-5, -1),
        (Key::S, _) => (-4, -1),
        (Key::D, _) => (-3, -1),
        (Key::F, _) => (-2, -1),
        (Key::G, _) => (-1, -1),
        (Key::H, _) => (0, -1),
        (Key::J, _) => (1, -1),
        (Key::K, _) => (2, -1),
        (Key::L, _) => (3, -1),
        (Key::Semicolon, _) => (4, -1),
        (Key::Apostrophe, _) => (5, -1),
        (Key::Return, KeyboardLayout::Ansi)
        | (Key::Return, KeyboardLayout::Variant)
        | (Key::Backslash, KeyboardLayout::Iso) => (6, -1),
        // ---
        (Key::LShift, KeyboardLayout::Iso) => (-7, -2),
        (Key::LShift, KeyboardLayout::Ansi)
        | (Key::LShift, KeyboardLayout::Variant)
        | (Key::Unlabeled, KeyboardLayout::Iso) => (-6, -2),
        (Key::Z, _) => (-5, -2),
        (Key::X, _) => (-4, -2),
        (Key::C, _) => (-3, -2),
        (Key::V, _) => (-2, -2),
        (Key::B, _) => (-1, -2),
        (Key::N, _) => (0, -2),
        (Key::M, _) => (1, -2),
        (Key::Comma, _) => (2, -2),
        (Key::Period, _) => (3, -2),
        (Key::Slash, _) => (4, -2),
        (Key::RShift, _) => (5, -2),
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
                "Invalid keyboard layout. Should be `{}`, `{}` or `{}`.",
                ANSI, VAR, ISO
            )),
        }
    }
}
