use nannou::prelude::Key;

pub fn hex_location_for_iso_keyboard(scancode: u32, virtual_key: Option<Key>) -> Option<(i8, i8)> {
    let physical_key = if cfg!(target_arch = "wasm32") {
        // We treat virtual keys as physical keys since winit(wasm32) confounds scancodes and virtual keycodes
        virtual_key
    } else {
        key_for_scancode(scancode)
    };

    physical_key.and_then(hex_location_for_iso_key)
}

fn key_for_scancode(keycode: u32) -> Option<Key> {
    Some(match keycode {
        41 => Key::Grave,
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
        14 => Key::Back,
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
        28 => Key::Return,
        // ---
        58 => Key::Capital,
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
        40 => Key::Apostrophe,
        43 => Key::Backslash,
        // ---
        42 => Key::LShift,
        86 => Key::Unlabeled,
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

fn hex_location_for_iso_key(physical_key: Key) -> Option<(i8, i8)> {
    Some(match physical_key {
        Key::Grave => (-6, 1), // web: Backquote
        Key::Key1 => (-5, 1),
        Key::Key2 => (-4, 1),
        Key::Key3 => (-3, 1),
        Key::Key4 => (-2, 1),
        Key::Key5 => (-1, 1),
        Key::Key6 => (0, 1),
        Key::Key7 => (1, 1),
        Key::Key8 => (2, 1),
        Key::Key9 => (3, 1),
        Key::Key0 => (4, 1),
        Key::Minus => (5, 1),
        Key::Equals => (6, 1),
        Key::Back => (7, 1), // web: Backspace
        // ---
        Key::Tab => (-6, 0),
        Key::Q => (-5, 0),
        Key::W => (-4, 0),
        Key::E => (-3, 0),
        Key::R => (-2, 0),
        Key::T => (-1, 0),
        Key::Y => (0, 0),
        Key::U => (1, 0),
        Key::I => (2, 0),
        Key::O => (3, 0),
        Key::P => (4, 0),
        Key::LBracket => (5, 0),
        Key::RBracket => (6, 0),
        Key::Return => (7, 0), // web: Enter
        // ---
        Key::Capital => (-6, -1), // web: CapsLock - ignored by winit
        Key::A => (-5, -1),
        Key::S => (-4, -1),
        Key::D => (-3, -1),
        Key::F => (-2, -1),
        Key::G => (-1, -1),
        Key::H => (0, -1),
        Key::J => (1, -1),
        Key::K => (2, -1),
        Key::L => (3, -1),
        Key::Semicolon => (4, -1),
        Key::Apostrophe => (5, -1), // web: Quote
        Key::Backslash => (6, -1),
        // ---
        Key::LShift => (-7, -2),
        Key::Unlabeled => (-6, -2), // web: IntlBackslash - ignored by winit
        Key::Z => (-5, -2),
        Key::X => (-4, -2),
        Key::C => (-3, -2),
        Key::V => (-2, -2),
        Key::B => (-1, -2),
        Key::N => (0, -2),
        Key::M => (1, -2),
        Key::Comma => (2, -2),
        Key::Period => (3, -2),
        Key::Slash => (4, -2),
        Key::RShift => (5, -2),
        _ => return None,
    })
}
