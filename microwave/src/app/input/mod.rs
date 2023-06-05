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
    app::model::{PianoEngineResource, ViewModel},
    control::LiveParameter,
    piano::{Event, Location, PianoEngine, SourceId},
    PhysicalKeyboardLayout,
};

use super::{Toggle, VirtualKeyboardLayout};

mod hex_layout;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input_event);
    }
}

fn handle_input_event(
    engine: Res<PianoEngineResource>,
    physical_layout: Res<PhysicalKeyboardLayout>,
    mut virtual_layout: ResMut<Toggle<VirtualKeyboardLayout>>,
    mut view_model: ResMut<ViewModel>,
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
                virtual_layout.curr_option(),
                mod_pressed,
                keyboard_input.key_code,
                keyboard_input.state,
            );
        }

        if keyboard_input.state.is_pressed() {
            handle_key_event(
                &engine.0,
                &mut virtual_layout,
                &mut view_model,
                &keyboard_input.logical_key,
                alt_pressed,
            );
        }
    }

    for mouse_button_input in mouse_button_inputs.read() {
        handle_mouse_button_event(&engine.0, window, &view_model, *mouse_button_input);
    }

    if !mouse_motions.is_empty() {
        handle_mouse_motion_event(&engine.0, window, &view_model);
    }

    for mouse_wheel in mouse_wheels.read() {
        handle_mouse_wheel_event(alt_pressed, &mut view_model, *mouse_wheel);
    }

    for touch_input in touch_inputs.read() {
        handle_touch_event(&engine.0, window, &view_model, *touch_input);
    }
}

fn handle_scan_code_event(
    engine: &PianoEngine,
    physical_layout: &PhysicalKeyboardLayout,
    virtual_layout: &VirtualKeyboardLayout,
    mod_pressed: bool,
    key_code: KeyCode,
    button_state: ButtonState,
) {
    if button_state.is_pressed() && mod_pressed {
        return;
    }

    if let Some(key_coord) = hex_layout::location_of_key(physical_layout, key_code) {
        let (x, y) = key_coord;
        let degree = virtual_layout.keyboard.get_key(x.into(), y.into());

        let event = match button_state {
            ButtonState::Pressed => {
                Event::Pressed(SourceId::Keyboard(x, y), Location::Degree(degree), 100)
            }
            ButtonState::Released => Event::Released(SourceId::Keyboard(x, y), 100),
        };

        engine.handle_event(event)
    }
}

fn handle_key_event(
    engine: &PianoEngine,
    virtual_layout: &mut ResMut<Toggle<VirtualKeyboardLayout>>,
    view_settings: &mut ResMut<ViewModel>,
    logical_key: &Key,
    alt_pressed: bool,
) {
    match logical_key {
        Key::Character(character) => match character.to_uppercase().as_str() {
            "E" if alt_pressed => engine.toggle_envelope_type(),
            "K" if alt_pressed => view_settings.on_screen_keyboards.toggle_next(),
            "L" if alt_pressed => engine.toggle_parameter(LiveParameter::Legato),
            "O" if alt_pressed => engine.toggle_synth_mode(),
            "T" if alt_pressed => engine.toggle_tuning_mode(),
            "Y" if alt_pressed => virtual_layout.toggle_next(),
            _ => {}
        },
        Key::F1 => engine.toggle_parameter(LiveParameter::Sound1),
        Key::F2 => engine.toggle_parameter(LiveParameter::Sound2),
        Key::F3 => engine.toggle_parameter(LiveParameter::Sound3),
        Key::F4 => engine.toggle_parameter(LiveParameter::Sound4),
        Key::F5 => engine.toggle_parameter(LiveParameter::Sound5),
        Key::F6 => engine.toggle_parameter(LiveParameter::Sound6),
        Key::F7 => engine.toggle_parameter(LiveParameter::Sound7),
        Key::F8 => engine.toggle_parameter(LiveParameter::Sound8),
        Key::F9 => engine.toggle_parameter(LiveParameter::Sound9),
        Key::F10 => engine.toggle_parameter(LiveParameter::Sound10),
        Key::Space => engine.toggle_parameter(LiveParameter::Foot),
        Key::ArrowUp if !alt_pressed => engine.dec_program(),
        Key::ArrowDown if !alt_pressed => engine.inc_program(),
        Key::ArrowLeft if alt_pressed => engine.change_ref_note_by(-1),
        Key::ArrowRight if alt_pressed => engine.change_ref_note_by(1),
        Key::ArrowLeft if !alt_pressed => engine.change_root_offset_by(-1),
        Key::ArrowRight if !alt_pressed => engine.change_root_offset_by(1),
        _ => {}
    }
}

fn handle_mouse_button_event(
    engine: &PianoEngine,
    window: &Window,
    view_model: &ViewModel,
    mouse_button_input: MouseButtonInput,
) {
    if mouse_button_input.button == MouseButton::Left {
        match mouse_button_input.state {
            ButtonState::Pressed => {
                if let Some(cursor_position) = window.cursor_position() {
                    handle_position_event(
                        engine,
                        window,
                        view_model,
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

fn handle_mouse_motion_event(engine: &PianoEngine, window: &Window, view_model: &ViewModel) {
    if let Some(cursor_position) = window.cursor_position() {
        handle_position_event(
            engine,
            window,
            view_model,
            cursor_position,
            SourceId::Mouse,
            |location| Event::Moved(SourceId::Mouse, location),
        );
    }
}

fn handle_mouse_wheel_event(
    alt_pressed: bool,
    view_model: &mut ResMut<ViewModel>,
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
        let shift_ratio = view_model.pitch_range().repeated(-x_delta / 500.0);
        view_model.viewport_left = view_model.viewport_left * shift_ratio;
        view_model.viewport_right = view_model.viewport_right * shift_ratio;
    } else {
        let zoom_ratio = Ratio::from_semitones(y_delta / 10.0);
        view_model.viewport_left = view_model.viewport_left * zoom_ratio;
        view_model.viewport_right = view_model.viewport_right / zoom_ratio;
    }

    let mut target_pitch_range = view_model.pitch_range();

    let min_pitch = Pitch::from_hz(20.0);
    let max_pitch = Pitch::from_hz(20000.0);
    let min_allowed_pitch_range = Ratio::from_octaves(1.0);
    let max_allowed_pitch_range = Ratio::between_pitches(min_pitch, max_pitch);

    if target_pitch_range < min_allowed_pitch_range {
        let x = target_pitch_range
            .stretched_by(min_allowed_pitch_range.inv())
            .divided_into_equal_steps(2.0);
        view_model.viewport_left = view_model.viewport_left * x;
        view_model.viewport_right = view_model.viewport_right / x;
    }

    if target_pitch_range > max_allowed_pitch_range {
        target_pitch_range = max_allowed_pitch_range;
    }

    if view_model.viewport_left < min_pitch {
        view_model.viewport_left = min_pitch;
        view_model.viewport_right = min_pitch * target_pitch_range;
    }

    if view_model.viewport_right > max_pitch {
        view_model.viewport_left = max_pitch / target_pitch_range;
        view_model.viewport_right = max_pitch;
    }
}

fn handle_touch_event(
    engine: &PianoEngine,
    window: &Window,
    view_model: &ViewModel,
    event: TouchInput,
) {
    let id = SourceId::Touchpad(event.id);

    match event.phase {
        TouchPhase::Started => {
            handle_position_event(engine, window, view_model, event.position, id, |location| {
                Event::Pressed(id, location, 100)
            })
        }
        TouchPhase::Moved => {
            handle_position_event(engine, window, view_model, event.position, id, |location| {
                Event::Moved(id, location)
            });
        }
        TouchPhase::Ended | TouchPhase::Canceled => engine.handle_event(Event::Released(id, 100)),
    }
}

fn handle_position_event(
    engine: &PianoEngine,
    window: &Window,
    view_model: &ViewModel,
    position: Vec2,
    id: SourceId,
    to_event: impl Fn(Location) -> Event,
) {
    let x_normalized = f64::from(position.x / window.width());
    let y_normalized = 1.0 - f64::from(position.y / window.height()).max(0.0).min(1.0);

    let keyboard_range = view_model.pitch_range();
    let pitch = view_model.viewport_left * keyboard_range.repeated(x_normalized);

    engine.handle_event(to_event(Location::Pitch(pitch)));
    match id {
        SourceId::Mouse => engine.set_parameter(LiveParameter::Breath, y_normalized),
        SourceId::Touchpad(_) => {
            engine.set_key_pressure(id, y_normalized);
        }
        SourceId::Keyboard(_, _) | SourceId::Midi(_) => unreachable!(),
    }
}
