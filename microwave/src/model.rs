use crate::{
    audio::AudioModel,
    piano::{PianoEngine, PianoEngineSnapshot},
};
use midir::MidiInputConnection;
use nannou::{
    event::{ElementState, KeyboardInput},
    prelude::*,
    winit::event::WindowEvent,
};
use std::{
    collections::HashSet,
    convert::TryFrom,
    ops::Deref,
    sync::{mpsc::Receiver, Arc},
};
use tune::{
    key::Keyboard,
    note::NoteLetter,
    pitch::{Pitch, Pitched},
    ratio::Ratio,
};

pub struct Model {
    pub audio: AudioModel<EventId>,
    pub delay_active: bool,
    pub rotary_active: bool,
    pub rotary_motor_voltage: f32,
    pub recording_active: bool,
    pub engine: Arc<PianoEngine>,
    pub engine_snapshot: PianoEngineSnapshot,
    pub keyboard: Keyboard,
    pub limit: u16,
    pub midi_in: Option<MidiInputConnection<()>>,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pub pressed_physical_keys: HashSet<(i8, i8)>,
    pub selected_program: SelectedProgram,
    pub program_updates: Receiver<SelectedProgram>,
}

pub struct SelectedProgram {
    pub program_number: u8,
    pub program_name: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum EventId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Midi(u8),
}

pub enum EventPhase {
    Pressed(u8),
    Moved,
    Released(u8),
}

impl Model {
    pub fn new(
        audio: AudioModel<EventId>,
        engine: Arc<PianoEngine>,
        engine_snapshot: PianoEngineSnapshot,
        keyboard: Keyboard,
        limit: u16,
        midi_in: Option<MidiInputConnection<()>>,
        program_updates: Receiver<SelectedProgram>,
    ) -> Self {
        let lowest_note = NoteLetter::Fsh.in_octave(2).pitch();
        let highest_note = NoteLetter::Ash.in_octave(5).pitch();
        Self {
            audio,
            delay_active: true,
            rotary_active: false,
            rotary_motor_voltage: 0.0,
            recording_active: false,
            engine,
            engine_snapshot,
            keyboard,
            limit,
            midi_in,
            lowest_note,
            highest_note,
            pressed_physical_keys: HashSet::new(),
            selected_program: SelectedProgram {
                program_number: 0,
                program_name: None,
            },
            program_updates,
        }
    }

    pub fn update(&mut self) {
        for update in self.program_updates.try_iter() {
            self.selected_program = update
        }
        self.engine.take_snapshot(&mut self.engine_snapshot);
    }

    pub fn keyboard_event(&mut self, (x, y): (i8, i8), pressed: bool) {
        let key_number = self.keyboard.get_key(x.into(), y.into()).midi_number();

        let (phase, net_change) = if pressed {
            (
                EventPhase::Pressed(100),
                self.pressed_physical_keys.insert((x, y)),
            )
        } else {
            (
                EventPhase::Released(100),
                self.pressed_physical_keys.remove(&(x, y)),
            )
        };

        // While a key is held down the pressed event is sent repeatedly. We ignore this case by checking net_change
        if net_change {
            self.engine
                .handle_key_offset_event(EventId::Keyboard(x, y), key_number, phase);
        }
    }

    pub fn toggle_delay(&mut self) {
        self.delay_active = !self.delay_active;
        self.audio.set_delay_active(self.delay_active);
    }

    pub fn toggle_rotary(&mut self) {
        self.rotary_active = !self.rotary_active;
        self.audio.set_rotary_active(self.rotary_active);
    }

    pub fn toggle_rotary_motor(&mut self) {
        if self.rotary_active {
            self.rotary_motor_voltage = if self.rotary_motor_voltage < 0.999 {
                1.0
            } else {
                0.0
            };
            self.audio
                .set_rotary_motor_voltage(self.rotary_motor_voltage);
        }
    }

    fn toggle_recording(&mut self) {
        self.recording_active = !self.recording_active;
        self.audio.set_recording_active(self.recording_active);
    }
}

impl Deref for Model {
    type Target = PianoEngineSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.engine_snapshot
    }
}

pub fn raw_event(app: &App, model: &mut Model, event: &WindowEvent) {
    if app.keys.mods.alt() {
        return;
    }

    if let WindowEvent::KeyboardInput {
        input: KeyboardInput {
            scancode, state, ..
        },
        ..
    } = event
    {
        if let Some(key_coord) = hex_location_for_iso_keyboard(*scancode) {
            let pressed = match state {
                ElementState::Pressed => true,
                ElementState::Released => false,
            };

            model.keyboard_event(key_coord, pressed);
        }
    }
}

fn hex_location_for_iso_keyboard(keycode: u32) -> Option<(i8, i8)> {
    let keycode = match i8::try_from(keycode) {
        Ok(keycode) => keycode,
        Err(_) => return None,
    };
    let key_coord = match keycode {
        41 => (keycode - 47, 1),       // Key before <1>
        2..=14 => (keycode - 7, 1),    // <1>..<BSP>
        15..=28 => (keycode - 21, 0),  // <TAB>..<RETURN>
        58 => (keycode - 64, -1),      // <CAPS>
        30..=40 => (keycode - 35, -1), // <A>..Second key after <L>
        43 => (keycode - 37, -1),      // Third key after <L>
        42 => (keycode - 49, -2),      // Left <SHIFT>
        86 => (keycode - 92, -2),      // Key before <Z>
        44..=54 => (keycode - 49, -2), // Z..Right <SHIFT>
        _ => return None,
    };
    Some(key_coord)
}

pub fn key_pressed(app: &App, model: &mut Model, key: Key) {
    let alt_pressed = app.keys.mods.alt();
    let ctrl_pressed = app.keys.mods.ctrl();
    let engine = &model.engine;
    match key {
        Key::C if alt_pressed => engine.toggle_continuous(),
        Key::E if alt_pressed => engine.toggle_envelope_type(),
        Key::O if alt_pressed => engine.toggle_synth_mode(),
        Key::L if alt_pressed => engine.toggle_legato(),
        Key::F9 if ctrl_pressed => model.toggle_delay(),
        Key::F10 if ctrl_pressed => model.toggle_rotary(),
        Key::F10 if !ctrl_pressed => model.toggle_rotary_motor(),
        Key::Space => model.toggle_recording(),
        Key::Up if !alt_pressed => engine.dec_program(&mut model.selected_program.program_number),
        Key::Down if !alt_pressed => engine.inc_program(&mut model.selected_program.program_number),
        Key::Left => engine.dec_root_note(),
        Key::Right => engine.inc_root_note(),
        _ => {}
    }
}

pub fn mouse_pressed(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        mouse_event(app, model, EventPhase::Pressed(100), app.mouse.position());
    }
}

pub fn mouse_moved(app: &App, model: &mut Model, position: Point2) {
    mouse_event(app, model, EventPhase::Moved, position);
}

pub fn mouse_released(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        mouse_event(app, model, EventPhase::Released(100), app.mouse.position());
    }
}

pub fn mouse_wheel(
    app: &App,
    model: &mut Model,
    mouse_scroll_delta: MouseScrollDelta,
    _: TouchPhase,
) {
    let (mut x_delta, mut y_delta) = match mouse_scroll_delta {
        MouseScrollDelta::LineDelta(x, y) => (10.0 * x as f64, 10.0 * y as f64),
        MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
    };

    if app.keys.mods.alt() {
        let tmp = x_delta;
        x_delta = -y_delta;
        y_delta = tmp;
    }

    if x_delta.abs() > y_delta.abs() {
        let ratio =
            Ratio::between_pitches(model.lowest_note, model.highest_note).repeated(x_delta / 500.0);
        model.lowest_note = model.lowest_note * ratio;
        model.highest_note = model.highest_note * ratio;
    } else {
        let ratio = Ratio::from_semitones(y_delta / 10.0);
        let lowest = model.lowest_note * ratio;
        let highest = model.highest_note / ratio;
        if lowest < highest {
            model.lowest_note = lowest;
            model.highest_note = highest;
        }
    }
}

fn mouse_event(app: &App, model: &mut Model, phase: EventPhase, mut position: Point2) {
    position.x = position.x / app.window_rect().w() + 0.5;
    position.y = position.y / app.window_rect().h() + 0.5;
    position_event(model, EventId::Mouse, position, phase);
}

pub fn touch(app: &App, model: &mut Model, event: TouchEvent) {
    let mut position = event.position;
    position.x = position.x / app.window_rect().w() + 0.5;
    position.y = position.y / app.window_rect().h() + 0.5;
    let phase = match event.phase {
        TouchPhase::Started => EventPhase::Pressed(100),
        TouchPhase::Moved => EventPhase::Moved,
        TouchPhase::Ended | TouchPhase::Cancelled => EventPhase::Released(100),
    };
    position_event(model, EventId::Touchpad(event.id), position, phase);
}

fn position_event(model: &Model, id: EventId, position: Vector2, phase: EventPhase) {
    let keyboard_range = Ratio::between_pitches(model.lowest_note, model.highest_note);
    let pitch = model.lowest_note * keyboard_range.repeated(position.x);
    model.engine.handle_pitch_event(id, pitch, phase);
}

pub fn update(_: &App, model: &mut Model, _: Update) {
    model.update()
}
