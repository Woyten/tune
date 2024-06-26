mod hex_layout;

use std::collections::HashSet;

use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
        touch::TouchPhase,
        ButtonState,
    },
    prelude::*,
};
use tune::pitch::{Pitch, Ratio};

use crate::{
    app::{
        resources::{HudStackResource, MainViewResource, PianoEngineResource},
        VirtualKeyboardResource,
    },
    control::LiveParameter,
    piano::{Event, Location, PianoEngine, SourceId},
    PhysicalKeyboardLayout,
};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input_event);
    }
}

fn handle_input_event(
    engine: Res<PianoEngineResource>,
    mut hud_stack: ResMut<HudStackResource>,
    physical_layout: Res<PhysicalKeyboardLayout>,
    mut virtual_keyboard: ResMut<VirtualKeyboardResource>,
    mut main_view: ResMut<MainViewResource>,
    windows: Query<&Window>,
    key_code: Res<ButtonInput<KeyCode>>,
    mut keyboard_inputs: EventReader<KeyboardInput>,
    mut mouse_button_inputs: EventReader<MouseButtonInput>,
    mouse_motions: EventReader<MouseMotion>,
    mut mouse_wheels: EventReader<MouseWheel>,
    mut touch_inputs: EventReader<TouchInput>,
    mut pressed_physical_keys: Local<HashSet<KeyCode>>,
) {
    let window = windows.single();
    let ctrl_pressed =
        key_code.pressed(KeyCode::ControlLeft) || key_code.pressed(KeyCode::ControlRight);
    let alt_pressed = key_code.pressed(KeyCode::AltLeft) || key_code.pressed(KeyCode::AltRight);
    let mod_pressed = ctrl_pressed || alt_pressed;

    for keyboard_input in keyboard_inputs.read() {
        let net_change = match keyboard_input.state {
            ButtonState::Pressed => pressed_physical_keys.insert(keyboard_input.key_code),
            ButtonState::Released => pressed_physical_keys.remove(&keyboard_input.key_code),
        };

        if net_change {
            handle_scan_code_event(
                &engine.0,
                &physical_layout,
                &virtual_keyboard,
                mod_pressed,
                keyboard_input.key_code,
                keyboard_input.state,
            );
        }

        if keyboard_input.state.is_pressed() {
            handle_key_event(
                &engine.0,
                &mut hud_stack,
                &mut virtual_keyboard,
                &keyboard_input.logical_key,
                alt_pressed,
            );
        }
    }

    for mouse_button_input in mouse_button_inputs.read() {
        handle_mouse_button_event(&engine.0, window, &main_view, *mouse_button_input);
    }

    if !mouse_motions.is_empty() {
        handle_mouse_motion_event(&engine.0, window, &main_view);
    }

    for mouse_wheel in mouse_wheels.read() {
        handle_mouse_wheel_event(alt_pressed, &mut main_view, *mouse_wheel);
    }

    for touch_input in touch_inputs.read() {
        handle_touch_event(&engine.0, window, &main_view, *touch_input);
    }
}

fn handle_scan_code_event(
    engine: &PianoEngine,
    physical_layout: &PhysicalKeyboardLayout,
    virtual_keyboard: &VirtualKeyboardResource,
    mod_pressed: bool,
    key_code: KeyCode,
    button_state: ButtonState,
) {
    if button_state.is_pressed() && mod_pressed {
        return;
    }

    if let Some(key_coord) = hex_layout::location_of_key(physical_layout, key_code) {
        let (x, y) = key_coord;
        let degree = virtual_keyboard.get_key(x.into(), y.into());

        let event = match button_state {
            ButtonState::Pressed => {
                Event::Pressed(SourceId::Keyboard(x, y), Location::Degree(degree), 100)
            }
            ButtonState::Released => Event::Released(SourceId::Keyboard(x, y), 100),
        };

        engine.handle_event(event)
    }
}

pub enum HudMode {
    Keyboard,
}

fn handle_key_event(
    engine: &PianoEngine,
    hud_stack: &mut ResMut<HudStackResource>,
    virtual_keyboard: &mut ResMut<VirtualKeyboardResource>,
    logical_key: &Key,
    alt_pressed: bool,
) {
    match (logical_key, alt_pressed) {
        (Key::F1, false) => engine.toggle_parameter(LiveParameter::Sound1),
        (Key::F2, false) => engine.toggle_parameter(LiveParameter::Sound2),
        (Key::F3, false) => engine.toggle_parameter(LiveParameter::Sound3),
        (Key::F4, false) => engine.toggle_parameter(LiveParameter::Sound4),
        (Key::F5, false) => engine.toggle_parameter(LiveParameter::Sound5),
        (Key::F6, false) => engine.toggle_parameter(LiveParameter::Sound6),
        (Key::F7, false) => engine.toggle_parameter(LiveParameter::Sound7),
        (Key::F8, false) => engine.toggle_parameter(LiveParameter::Sound8),
        (Key::F9, false) => engine.toggle_parameter(LiveParameter::Sound9),
        (Key::F10, false) => engine.toggle_parameter(LiveParameter::Sound10),
        (Key::Space, false) => engine.toggle_parameter(LiveParameter::Foot),
        (Key::ArrowUp, true) => engine.dec_backend(),
        (Key::ArrowDown, true) => engine.inc_backend(),
        (Key::ArrowUp, false) => engine.dec_program(),
        (Key::ArrowDown, false) => engine.inc_program(),
        (Key::ArrowLeft, true) => engine.change_ref_note_by(-1),
        (Key::ArrowRight, true) => engine.change_ref_note_by(1),
        (Key::ArrowLeft, false) => engine.change_root_offset_by(-1),
        (Key::ArrowRight, false) => engine.change_root_offset_by(1),
        (Key::Character(character), true) => {
            let character = &character.to_uppercase();
            match hud_stack.top() {
                None => match &**character {
                    "E" if alt_pressed => engine.toggle_envelope_type(),
                    "K" => hud_stack.push(HudMode::Keyboard),
                    "L" => engine.toggle_parameter(LiveParameter::Legato),
                    "T" => engine.toggle_tuning_mode(),
                    _ => {}
                },
                Some(HudMode::Keyboard) => {
                    match &**character {
                        "C" => virtual_keyboard.compression.toggle_next(),
                        "I" => virtual_keyboard.inclination.toggle_next(),
                        "K" => virtual_keyboard.on_screen_keyboard.toggle_next(),
                        "L" => virtual_keyboard.layout.toggle_next(),
                        "S" => virtual_keyboard.scale.toggle_next(),
                        "T" => virtual_keyboard.tilt.toggle_next(),
                        _ => {}
                    };
                }
            }
        }
        (Key::Escape, false) => {
            hud_stack.pop();
        }
        _ => {}
    }
}

fn handle_mouse_button_event(
    engine: &PianoEngine,
    window: &Window,
    main_view: &MainViewResource,
    mouse_button_input: MouseButtonInput,
) {
    if mouse_button_input.button == MouseButton::Left {
        match mouse_button_input.state {
            ButtonState::Pressed => {
                if let Some(cursor_position) = window.cursor_position() {
                    handle_position_event(
                        engine,
                        window,
                        main_view,
                        cursor_position,
                        SourceId::Mouse,
                        |location| Event::Pressed(SourceId::Mouse, location, 100),
                    )
                }
            }
            ButtonState::Released => engine.handle_event(Event::Released(SourceId::Mouse, 100)),
        }
    }
}

fn handle_mouse_motion_event(engine: &PianoEngine, window: &Window, main_view: &MainViewResource) {
    if let Some(cursor_position) = window.cursor_position() {
        handle_position_event(
            engine,
            window,
            main_view,
            cursor_position,
            SourceId::Mouse,
            |location| Event::Moved(SourceId::Mouse, location),
        );
    }
}

fn handle_mouse_wheel_event(
    alt_pressed: bool,
    main_view: &mut ResMut<MainViewResource>,
    mouse_wheel: MouseWheel,
) {
    let unit_factor = match mouse_wheel.unit {
        MouseScrollUnit::Line => 10.0,
        MouseScrollUnit::Pixel => 0.05,
    };

    let mut x_delta = mouse_wheel.x * unit_factor;
    let mut y_delta = mouse_wheel.y * unit_factor;

    if alt_pressed {
        (x_delta, y_delta) = (y_delta, -x_delta);
    }

    if x_delta.abs() > y_delta.abs() {
        let shift_ratio = main_view.pitch_range().repeated(-x_delta / 500.0);
        main_view.viewport_left = main_view.viewport_left * shift_ratio;
        main_view.viewport_right = main_view.viewport_right * shift_ratio;
    } else {
        let zoom_ratio = Ratio::from_semitones(y_delta / 10.0);
        main_view.viewport_left = main_view.viewport_left * zoom_ratio;
        main_view.viewport_right = main_view.viewport_right / zoom_ratio;
    }

    let mut target_pitch_range = main_view.pitch_range();

    let min_pitch = Pitch::from_hz(20.0);
    let max_pitch = Pitch::from_hz(20000.0);
    let min_allowed_pitch_range = Ratio::from_octaves(1.0);
    let max_allowed_pitch_range = Ratio::between_pitches(min_pitch, max_pitch);

    if target_pitch_range < min_allowed_pitch_range {
        let x = target_pitch_range
            .stretched_by(min_allowed_pitch_range.inv())
            .divided_into_equal_steps(2.0);
        main_view.viewport_left = main_view.viewport_left * x;
        main_view.viewport_right = main_view.viewport_right / x;
    }

    if target_pitch_range > max_allowed_pitch_range {
        target_pitch_range = max_allowed_pitch_range;
    }

    if main_view.viewport_left < min_pitch {
        main_view.viewport_left = min_pitch;
        main_view.viewport_right = min_pitch * target_pitch_range;
    }

    if main_view.viewport_right > max_pitch {
        main_view.viewport_left = max_pitch / target_pitch_range;
        main_view.viewport_right = max_pitch;
    }
}

fn handle_touch_event(
    engine: &PianoEngine,
    window: &Window,
    main_view: &MainViewResource,
    event: TouchInput,
) {
    let id = SourceId::Touchpad(event.id);

    match event.phase {
        TouchPhase::Started => {
            handle_position_event(engine, window, main_view, event.position, id, |location| {
                Event::Pressed(id, location, 100)
            })
        }
        TouchPhase::Moved => {
            handle_position_event(engine, window, main_view, event.position, id, |location| {
                Event::Moved(id, location)
            });
        }
        TouchPhase::Ended | TouchPhase::Canceled => engine.handle_event(Event::Released(id, 100)),
    }
}

fn handle_position_event(
    engine: &PianoEngine,
    window: &Window,
    main_view: &MainViewResource,
    position: Vec2,
    id: SourceId,
    to_event: impl Fn(Location) -> Event,
) {
    let x_normalized = f64::from(position.x / window.width());
    let y_normalized = 1.0 - f64::from(position.y / window.height()).max(0.0).min(1.0);

    let keyboard_range = main_view.pitch_range();
    let pitch = main_view.viewport_left * keyboard_range.repeated(x_normalized);

    engine.handle_event(to_event(Location::Pitch(pitch)));
    match id {
        SourceId::Mouse => engine.set_parameter(LiveParameter::Breath, y_normalized),
        SourceId::Touchpad(..) => {
            engine.set_key_pressure(id, y_normalized);
        }
        SourceId::Keyboard(..) | SourceId::Midi(..) => unreachable!(),
    }
}
