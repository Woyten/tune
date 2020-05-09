use crate::audio::Audio;
use nannou::prelude::*;
use std::collections::HashMap;
use tune::{
    key::PianoKey,
    key_map::KeyMap,
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    ratio::Ratio,
    scale::Scale,
    tuning::Tuning,
};

pub struct Model {
    pub scale: Option<Scale>,
    pub root_note: Note,
    pub legato: bool,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pub mouse_event_id: u64,
    pub waveform: Waveform,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
    pub audio: Audio<EventId>,
}

#[derive(Copy, Clone)]
pub enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
}

pub struct VirtualKey {
    pub pitch: Pitch,
}

impl Model {
    pub fn new(scale: Option<Scale>) -> Self {
        Self {
            scale,
            legato: true,
            root_note: NoteLetter::D.in_octave(4),
            lowest_note: NoteLetter::Gsh.in_octave(2).pitch(),
            highest_note: NoteLetter::Gsh.in_octave(5).pitch(),
            mouse_event_id: 0,
            waveform: Waveform::Sine,
            pressed_keys: HashMap::new(),
            audio: Audio::new(),
        }
    }
}

struct VirtualKeyboardEvent {
    id: EventId,
    position: Point2,
    phase: EventPhase,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum EventId {
    Mouse(u64),
    Touchpad(u64),
}

enum EventPhase {
    Pressed,
    Moved,
    Released,
}

pub fn key_pressed(_app: &App, model: &mut Model, key: Key) {
    match key {
        Key::L => model.legato = !model.legato,
        Key::W => {
            model.waveform = match model.waveform {
                Waveform::Sine => Waveform::Triangle,
                Waveform::Triangle => Waveform::Square,
                Waveform::Square => Waveform::Sawtooth,
                Waveform::Sawtooth => Waveform::Sine,
            }
        }
        Key::Left => {
            model.root_note = Note::from_midi_number(model.root_note.midi_number() - 1);
        }
        Key::Right => {
            model.root_note = Note::from_midi_number(model.root_note.midi_number() + 1);
        }
        _ => {}
    }
}

pub fn mouse_pressed(app: &App, model: &mut Model, _: MouseButton) {
    mouse_event(app, model, EventPhase::Pressed, app.mouse.position());
}

pub fn mouse_moved(app: &App, model: &mut Model, position: Point2) {
    mouse_event(app, model, EventPhase::Moved, position);
}

pub fn mouse_released(app: &App, model: &mut Model, _: MouseButton) {
    mouse_event(app, model, EventPhase::Released, app.mouse.position());
    model.mouse_event_id += 1;
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

fn mouse_event(app: &App, model: &mut Model, phase: EventPhase, position: Point2) {
    let event = VirtualKeyboardEvent {
        id: EventId::Mouse(model.mouse_event_id),
        position,
        phase,
    };
    virtual_keyboard(app, model, event);
}

pub fn touch(app: &App, model: &mut Model, event: TouchEvent) {
    let phase = match event.phase {
        TouchPhase::Started => EventPhase::Pressed,
        TouchPhase::Moved => EventPhase::Moved,
        TouchPhase::Ended | TouchPhase::Cancelled => EventPhase::Released,
    };
    let event = VirtualKeyboardEvent {
        id: EventId::Touchpad(event.id),
        position: event.position,
        phase,
    };
    virtual_keyboard(app, model, event);
}

fn virtual_keyboard(app: &App, model: &mut Model, event: VirtualKeyboardEvent) {
    let x_position = event.position.x as f64 / app.window_rect().w() as f64 + 0.5;

    let keyboard_range = Ratio::between_pitches(model.lowest_note, model.highest_note);

    let mut pitch =
        model.lowest_note * Ratio::from_octaves(keyboard_range.as_octaves() * x_position);

    if let Some(scale) = &model.scale {
        let key_map = KeyMap::root_at(model.root_note);
        let scale_with_key_map = scale.with_key_map(&key_map);
        let key: PianoKey = scale_with_key_map.find_by_pitch(pitch).approx_value;
        pitch = scale_with_key_map.pitch_of(key);
    }

    let id = event.id;
    let waveform = model.waveform;

    match event.phase {
        EventPhase::Pressed => {
            model.audio.start(id, pitch, waveform);
            model.pressed_keys.insert(id, VirtualKey { pitch });
        }
        EventPhase::Moved if model.legato => {
            model.audio.update(id, pitch);
            if let Some(pressed_key) = model.pressed_keys.get_mut(&id) {
                pressed_key.pitch = pitch;
            }
        }
        EventPhase::Released => {
            model.audio.stop(id);
            model.pressed_keys.remove(&id);
        }
        _ => {}
    }
}
