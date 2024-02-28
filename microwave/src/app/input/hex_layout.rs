use bevy::prelude::*;

use crate::PhysicalKeyboardLayout;

pub fn location_of_key(
    physical_layout: &PhysicalKeyboardLayout,
    key_code: KeyCode,
) -> Option<(i8, i8)> {
    Some(match (key_code, physical_layout) {
        (KeyCode::Backquote, _) => (-6, -1),
        (KeyCode::Digit1, _) => (-5, -1),
        (KeyCode::Digit2, _) => (-4, -1),
        (KeyCode::Digit3, _) => (-3, -1),
        (KeyCode::Digit4, _) => (-2, -1),
        (KeyCode::Digit5, _) => (-1, -1),
        (KeyCode::Digit6, _) => (0, -1),
        (KeyCode::Digit7, _) => (1, -1),
        (KeyCode::Digit8, _) => (2, -1),
        (KeyCode::Digit9, _) => (3, -1),
        (KeyCode::Digit0, _) => (4, -1),
        (KeyCode::Minus, _) => (5, -1),
        (KeyCode::Equal, _) => (6, -1),
        (KeyCode::Backspace, PhysicalKeyboardLayout::Ansi)
        | (KeyCode::Backslash, PhysicalKeyboardLayout::Variant)
        | (KeyCode::Backspace, PhysicalKeyboardLayout::Iso) => (7, -1),
        (KeyCode::Backspace, PhysicalKeyboardLayout::Variant) => (8, -1),
        // ---
        (KeyCode::Tab, _) => (-6, 0),
        (KeyCode::KeyQ, _) => (-5, 0),
        (KeyCode::KeyW, _) => (-4, 0),
        (KeyCode::KeyE, _) => (-3, 0),
        (KeyCode::KeyR, _) => (-2, 0),
        (KeyCode::KeyT, _) => (-1, 0),
        (KeyCode::KeyY, _) => (0, 0),
        (KeyCode::KeyU, _) => (1, 0),
        (KeyCode::KeyI, _) => (2, 0),
        (KeyCode::KeyO, _) => (3, 0),
        (KeyCode::KeyP, _) => (4, 0),
        (KeyCode::BracketLeft, _) => (5, 0),
        (KeyCode::BracketRight, _) => (6, 0),
        (KeyCode::Backslash, PhysicalKeyboardLayout::Ansi)
        | (KeyCode::Enter, PhysicalKeyboardLayout::Iso) => (7, 0),
        // ---
        (KeyCode::CapsLock, _) => (-6, 1),
        (KeyCode::KeyA, _) => (-5, 1),
        (KeyCode::KeyS, _) => (-4, 1),
        (KeyCode::KeyD, _) => (-3, 1),
        (KeyCode::KeyF, _) => (-2, 1),
        (KeyCode::KeyG, _) => (-1, 1),
        (KeyCode::KeyH, _) => (0, 1),
        (KeyCode::KeyJ, _) => (1, 1),
        (KeyCode::KeyK, _) => (2, 1),
        (KeyCode::KeyL, _) => (3, 1),
        (KeyCode::Semicolon, _) => (4, 1),
        (KeyCode::Quote, _) => (5, 1),
        (KeyCode::Enter, PhysicalKeyboardLayout::Ansi)
        | (KeyCode::Enter, PhysicalKeyboardLayout::Variant)
        | (KeyCode::Backslash, PhysicalKeyboardLayout::Iso) => (6, 1),
        // ---
        (KeyCode::ShiftLeft, PhysicalKeyboardLayout::Iso) => (-7, 2),
        (KeyCode::ShiftLeft, PhysicalKeyboardLayout::Ansi)
        | (KeyCode::ShiftLeft, PhysicalKeyboardLayout::Variant)
        | (KeyCode::IntlBackslash, PhysicalKeyboardLayout::Iso) => (-6, 2),
        (KeyCode::KeyZ, _) => (-5, 2),
        (KeyCode::KeyX, _) => (-4, 2),
        (KeyCode::KeyC, _) => (-3, 2),
        (KeyCode::KeyV, _) => (-2, 2),
        (KeyCode::KeyB, _) => (-1, 2),
        (KeyCode::KeyN, _) => (0, 2),
        (KeyCode::KeyM, _) => (1, 2),
        (KeyCode::Comma, _) => (2, 2),
        (KeyCode::Period, _) => (3, 2),
        (KeyCode::Slash, _) => (4, 2),
        (KeyCode::ShiftRight, _) => (5, 2),
        _ => return None,
    })
}
