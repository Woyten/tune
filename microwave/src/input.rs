use std::collections::HashSet;

use bevy::{
    input::{
        keyboard::KeyboardInput,
        mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
        touch::TouchPhase,
        ButtonState,
    },
    prelude::*,
};
use tune::pitch::{Pitch, Ratio};

use crate::{
    control::LiveParameter,
    keyboard,
    model::{Event, Location, Viewport},
    {Model, SourceId},
};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(handle_input_event);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_input_event(
    model: Res<Model>,
    mut viewport: ResMut<Viewport>,
    windows: Query<&Window>,
    key_code: Res<Input<KeyCode>>,
    mut keyboard_inputs: EventReader<KeyboardInput>,
    mut mouse_button_inputs: EventReader<MouseButtonInput>,
    mouse_motions: EventReader<MouseMotion>,
    mut mouse_wheels: EventReader<MouseWheel>,
    mut touch_inputs: EventReader<TouchInput>,
    mut pressed_physical_keys: Local<HashSet<u32>>,
) {
    let window = windows.single();

    let ctrl_pressed = key_code.pressed(KeyCode::LControl) || key_code.pressed(KeyCode::RControl);
    let alt_pressed = key_code.pressed(KeyCode::LAlt) || key_code.pressed(KeyCode::RAlt);
    let mod_pressed = ctrl_pressed || alt_pressed;

    for keyboard_input in keyboard_inputs.iter() {
        let net_change = match keyboard_input.state {
            ButtonState::Pressed => pressed_physical_keys.insert(keyboard_input.scan_code),
            ButtonState::Released => pressed_physical_keys.remove(&keyboard_input.scan_code),
        };

        if net_change {
            model.handle_scan_code_event(
                keyboard_input.scan_code,
                keyboard_input.key_code,
                keyboard_input.state,
                mod_pressed,
            );
        }

        if let Some(key_code) = keyboard_input.key_code {
            if keyboard_input.state.is_pressed() {
                model.handle_key_code_event(key_code, alt_pressed);
            }
        }
    }

    for mouse_button_input in mouse_button_inputs.iter() {
        model.handle_mouse_button_event(window, &viewport, *mouse_button_input);
    }

    if !mouse_motions.is_empty() {
        model.handle_mouse_motion_event(window, &viewport);
    }

    for mouse_wheel in mouse_wheels.iter() {
        Model::handle_mouse_wheel_event(&mut viewport, *mouse_wheel, alt_pressed);
    }

    for touch_input in touch_inputs.iter() {
        model.handle_touch_event(window, &viewport, *touch_input);
    }
}

impl Model {
    fn handle_scan_code_event(
        &self,
        scan_code: u32,
        key_code: Option<KeyCode>, // KeyCode only necessary because of winit(wasm32) bug
        button_state: ButtonState,
        mod_pressed: bool,
    ) {
        if button_state.is_pressed() && mod_pressed {
            return;
        }

        if let Some(key_coord) = keyboard::calc_hex_location(self.layout, scan_code, key_code) {
            let (x, y) = key_coord;
            let degree = self.keyboard.get_key(x.into(), y.into()).midi_number();

            let event = match button_state {
                ButtonState::Pressed => {
                    Event::Pressed(SourceId::Keyboard(x, y), Location::Degree(degree), 100)
                }
                ButtonState::Released => Event::Released(SourceId::Keyboard(x, y), 100),
            };

            self.engine.handle_event(event)
        }
    }

    fn handle_key_code_event(&self, key_code: KeyCode, alt_pressed: bool) {
        let engine = &self.engine;
        match key_code {
            KeyCode::T if alt_pressed => engine.toggle_tuning_mode(),
            KeyCode::E if alt_pressed => engine.toggle_envelope_type(),
            KeyCode::O if alt_pressed => engine.toggle_synth_mode(),
            KeyCode::L if alt_pressed => engine.toggle_parameter(LiveParameter::Legato),
            KeyCode::F1 => engine.toggle_parameter(LiveParameter::Sound1),
            KeyCode::F2 => engine.toggle_parameter(LiveParameter::Sound2),
            KeyCode::F3 => engine.toggle_parameter(LiveParameter::Sound3),
            KeyCode::F4 => engine.toggle_parameter(LiveParameter::Sound4),
            KeyCode::F5 => engine.toggle_parameter(LiveParameter::Sound5),
            KeyCode::F6 => engine.toggle_parameter(LiveParameter::Sound6),
            KeyCode::F7 => engine.toggle_parameter(LiveParameter::Sound7),
            KeyCode::F8 => engine.toggle_parameter(LiveParameter::Sound8),
            KeyCode::F9 => engine.toggle_parameter(LiveParameter::Sound9),
            KeyCode::F10 => engine.toggle_parameter(LiveParameter::Sound10),
            KeyCode::Space => engine.toggle_parameter(LiveParameter::Foot),
            KeyCode::Up if !alt_pressed => engine.dec_program(),
            KeyCode::Down if !alt_pressed => engine.inc_program(),
            KeyCode::Left if alt_pressed => engine.change_ref_note_by(-1),
            KeyCode::Right if alt_pressed => engine.change_ref_note_by(1),
            KeyCode::Left if !alt_pressed => engine.change_root_offset_by(-1),
            KeyCode::Right if !alt_pressed => engine.change_root_offset_by(1),
            _ => {}
        }
    }

    fn handle_mouse_button_event(
        &self,
        window: &Window,
        viewport: &Viewport,
        mouse_button_input: MouseButtonInput,
    ) {
        if mouse_button_input.button == MouseButton::Left {
            match mouse_button_input.state {
                ButtonState::Pressed => {
                    if let Some(cursor_position) = window.cursor_position() {
                        self.handle_position_event(
                            window,
                            viewport,
                            cursor_position,
                            SourceId::Mouse,
                            |location| Event::Pressed(SourceId::Mouse, location, 100),
                        )
                    }
                }
                ButtonState::Released => self
                    .engine
                    .handle_event(Event::Released(SourceId::Mouse, 100)),
            }
        }
    }

    fn handle_mouse_motion_event(&self, window: &Window, viewport: &Viewport) {
        if let Some(cursor_position) = window.cursor_position() {
            self.handle_position_event(
                window,
                viewport,
                cursor_position,
                SourceId::Mouse,
                |location| Event::Moved(SourceId::Mouse, location),
            );
        }
    }

    fn handle_mouse_wheel_event(
        viewport: &mut ResMut<Viewport>,
        mouse_wheel: MouseWheel,
        alt_pressed: bool,
    ) {
        let unit_factor = match mouse_wheel.unit {
            MouseScrollUnit::Line => 10.0,
            MouseScrollUnit::Pixel => 1.0,
        };

        let mut x_delta = mouse_wheel.x * unit_factor;
        let mut y_delta = mouse_wheel.y * unit_factor;

        if alt_pressed {
            (x_delta, y_delta) = (y_delta, -x_delta);
        }

        if x_delta.abs() > y_delta.abs() {
            let shift_ratio =
                Ratio::between_pitches(viewport.pitch_range.start, viewport.pitch_range.end)
                    .repeated(-x_delta / 500.0);
            viewport.pitch_range.start = viewport.pitch_range.start * shift_ratio;
            viewport.pitch_range.end = viewport.pitch_range.end * shift_ratio;
        } else {
            let zoom_ratio = Ratio::from_semitones(y_delta / 10.0);
            viewport.pitch_range.start = viewport.pitch_range.start * zoom_ratio;
            viewport.pitch_range.end = viewport.pitch_range.end / zoom_ratio;
        }

        let mut target_pitch_range =
            Ratio::between_pitches(viewport.pitch_range.start, viewport.pitch_range.end);

        let min_pitch = Pitch::from_hz(20.0);
        let max_pitch = Pitch::from_hz(20000.0);
        let min_allowed_pitch_range = Ratio::from_octaves(1.0);
        let max_allowed_pitch_range = Ratio::between_pitches(min_pitch, max_pitch);

        if target_pitch_range < min_allowed_pitch_range {
            let x = target_pitch_range
                .stretched_by(min_allowed_pitch_range.inv())
                .divided_into_equal_steps(2.0);
            viewport.pitch_range.start = viewport.pitch_range.start * x;
            viewport.pitch_range.end = viewport.pitch_range.end / x;
        }

        if target_pitch_range > max_allowed_pitch_range {
            target_pitch_range = max_allowed_pitch_range;
        }

        if viewport.pitch_range.start < min_pitch {
            viewport.pitch_range.start = min_pitch;
            viewport.pitch_range.end = min_pitch * target_pitch_range;
        }

        if viewport.pitch_range.end > max_pitch {
            viewport.pitch_range.start = max_pitch / target_pitch_range;
            viewport.pitch_range.end = max_pitch;
        }
    }

    fn handle_touch_event(&self, window: &Window, viewport: &Viewport, mut event: TouchInput) {
        let id = SourceId::Touchpad(event.id);
        event.position.y = window.height() - event.position.y;

        match event.phase {
            TouchPhase::Started => {
                self.handle_position_event(window, viewport, event.position, id, |location| {
                    Event::Pressed(id, location, 100)
                })
            }
            TouchPhase::Moved => {
                self.handle_position_event(window, viewport, event.position, id, |location| {
                    Event::Moved(id, location)
                });
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.engine.handle_event(Event::Released(id, 100))
            }
        }
    }

    fn handle_position_event(
        &self,
        window: &Window,
        viewport: &Viewport,
        position: Vec2,
        id: SourceId,
        to_event: impl Fn(Location) -> Event,
    ) {
        let x_normalized = f64::from(position.x / window.width());
        let y_normalized = f64::from(position.y / window.height()).clamp(0.0, 1.0);

        let keyboard_range =
            Ratio::between_pitches(viewport.pitch_range.start, viewport.pitch_range.end);
        let pitch = viewport.pitch_range.start * keyboard_range.repeated(x_normalized);

        match id {
            SourceId::Mouse => self
                .engine
                .set_parameter(LiveParameter::Breath, y_normalized),
            SourceId::Touchpad(_) => {
                self.engine.set_key_pressure(id, y_normalized);
            }
            SourceId::Keyboard(_, _) | SourceId::Midi(_) => unreachable!(),
        }
        self.engine.handle_event(to_event(Location::Pitch(pitch)));
    }
}
