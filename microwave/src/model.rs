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
    pub recording_active: bool,
    pub engine: Arc<PianoEngine>,
    pub engine_snapshot: PianoEngineSnapshot,
    pub keyboard: Keyboard,
    pub limit: u16,
    pub midi_in: Option<MidiInputConnection<()>>,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pub pressed_physical_keys: HashSet<u32>,
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
    Keyboard(u32),
    Midi(u8),
}

pub enum EventPhase {
    Pressed(u8),
    Moved,
    Released,
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
}

impl Model {
    fn toggle_recording(&mut self) {
        self.recording_active = !self.recording_active;
        if self.recording_active {
            self.audio.start_recording();
        } else {
            self.audio.stop_recording();
        }
    }
}

impl Deref for Model {
    type Target = PianoEngineSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.engine_snapshot
    }
}

pub fn event(app: &App, model: &mut Model, event: &WindowEvent) {
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
        if let Some(key_number) = hex_location_for_iso_keyboard(model, *scancode) {
            let (phase, net_change) = match state {
                ElementState::Pressed => (
                    EventPhase::Pressed(100),
                    model.pressed_physical_keys.insert(*scancode),
                ),
                ElementState::Released => (
                    EventPhase::Released,
                    model.pressed_physical_keys.remove(scancode),
                ),
            };

            // While a key is held down ElementState::Pressed is sent repeatedly. We ignore this case by checking net_change
            if net_change {
                model.engine.handle_key_offset_event(
                    EventId::Keyboard(*scancode),
                    key_number,
                    phase,
                );
            }
        }
    }
}

fn hex_location_for_iso_keyboard(model: &Model, keycode: u32) -> Option<i32> {
    let keycode = match i16::try_from(keycode) {
        Ok(keycode) => keycode,
        Err(_) => return None,
    };
    let (x, y) = match keycode {
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
    Some(model.keyboard.get_key(x, y).midi_number())
}

pub fn key_pressed(app: &App, model: &mut Model, key: Key) {
    let alt_pressed = app.keys.mods.alt();
    let engine = &model.engine;
    match key {
        Key::L if alt_pressed => engine.toggle_legato(),
        Key::C if alt_pressed => engine.toggle_continuous(),
        Key::E if app.keys.mods.alt() => engine.toggle_envelope_type(),
        Key::Space => model.toggle_recording(),
        Key::Up if !alt_pressed => engine.dec_program(&mut model.selected_program.program_number),
        Key::Down if !alt_pressed => engine.inc_program(&mut model.selected_program.program_number),
        Key::Up if alt_pressed => engine.toggle_synth_mode(),
        Key::Down if alt_pressed => engine.toggle_synth_mode(),
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
        mouse_event(app, model, EventPhase::Released, app.mouse.position());
    }
}

pub fn mouse_wheel(
    _: &App,
    model: &mut Model,
    mouse_scroll_delta: MouseScrollDelta,
    _: TouchPhase,
) {
    let (x_delta, y_delta) = match mouse_scroll_delta {
        MouseScrollDelta::LineDelta(x, y) => (x as f64, y as f64),
        MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
    };

    if x_delta.abs() > y_delta.abs() {
        model.lowest_note = model.lowest_note * Ratio::from_semitones(x_delta);
        model.highest_note = model.highest_note * Ratio::from_semitones(x_delta);
    } else {
        let lowest = model.lowest_note * Ratio::from_semitones(y_delta);
        let highest = model.highest_note / Ratio::from_semitones(y_delta);
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
        TouchPhase::Ended | TouchPhase::Cancelled => EventPhase::Released,
    };
    position_event(model, EventId::Touchpad(event.id), position, phase);
}

fn position_event(model: &Model, id: EventId, position: Vector2, phase: EventPhase) {
    let keyboard_range = Ratio::between_pitches(model.lowest_note, model.highest_note);
    let pitch = model.lowest_note * keyboard_range.repeated(position.x);
    model.engine.handle_pitch_event(id, pitch, phase);
}

pub fn update(_: &App, app_model: &mut Model, _: Update) {
    for update in app_model.program_updates.try_iter() {
        app_model.selected_program = update
    }
    app_model
        .engine
        .take_snapshot(&mut app_model.engine_snapshot);
}
