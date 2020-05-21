use crate::{
    audio::Audio,
    wave::{self, Patch},
};
use nannou::prelude::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tune::{
    key_map::KeyMap,
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    ratio::Ratio,
    scale::Scale,
    tuning::Tuning,
};

pub struct Model {
    pub synth_mode: SynthMode,
    pub soundfont_provided: bool,
    pub scale: Option<Scale>,
    pub root_note: Note,
    pub legato: bool,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pub mouse_event_id: u64,
    pub waveforms: Vec<Patch>,
    pub selected_waveform: usize,
    pub program_number: u32,
    pub program_name: Arc<Mutex<Option<String>>>,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
    pub audio: Audio<EventId>,
}

impl Model {
    pub fn new(
        scale: Option<Scale>,
        soundfont_file_location: Option<PathBuf>,
        program_number: u32,
    ) -> Self {
        let mut model = Self {
            synth_mode: if soundfont_file_location.is_some() {
                SynthMode::Fluid
            } else {
                SynthMode::Waveform
            },
            soundfont_provided: soundfont_file_location.is_some(),
            scale,
            root_note: NoteLetter::D.in_octave(4),
            legato: true,
            lowest_note: NoteLetter::Fsh.in_octave(2).pitch(),
            highest_note: NoteLetter::Ash.in_octave(5).pitch(),
            mouse_event_id: 0,
            waveforms: wave::all_waveforms(),
            selected_waveform: 0,
            program_number,
            program_name: Arc::new(Mutex::new(None)),
            pressed_keys: HashMap::new(),
            audio: Audio::new(soundfont_file_location),
        };
        model.retune();
        model.update_program();
        model
    }

    fn retune(&mut self) {
        if let Some(scale) = &mut self.scale {
            self.audio
                .retune(scale.with_key_map(&KeyMap::root_at(self.root_note)))
        };
    }

    fn update_program(&mut self) {
        self.audio
            .set_program(self.program_number, self.program_name.clone());
    }
}

pub enum SynthMode {
    Waveform,
    Fluid,
}

pub struct VirtualKey {
    pub pitch: Pitch,
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
        Key::Space => {
            if model.soundfont_provided {
                model.synth_mode = match model.synth_mode {
                    SynthMode::Waveform => SynthMode::Fluid,
                    SynthMode::Fluid => SynthMode::Waveform,
                }
            }
        }
        Key::Up => match model.synth_mode {
            SynthMode::Waveform => {
                model.selected_waveform =
                    (model.selected_waveform + model.waveforms.len() - 1) % model.waveforms.len();
            }
            SynthMode::Fluid => {
                model.program_number = (model.program_number + 128 - 1) % 128;
                model.update_program();
            }
        },
        Key::Down => match model.synth_mode {
            SynthMode::Waveform => {
                model.selected_waveform = (model.selected_waveform + 1) % model.waveforms.len();
            }
            SynthMode::Fluid => {
                model.program_number = (model.program_number + 1) % 128;
                model.update_program();
            }
        },
        Key::Left => {
            model.root_note = model.root_note.plus_semitones(-1);
            model.retune();
        }
        Key::Right => {
            model.root_note = model.root_note.plus_semitones(1);
            model.retune();
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

    let key = if let Some(scale) = &model.scale {
        let key_map = KeyMap::root_at(model.root_note);
        let scale_with_key_map = scale.with_key_map(&key_map);
        let key = scale_with_key_map.find_by_pitch(pitch).approx_value;
        pitch = scale_with_key_map.pitch_of(key);
        key
    } else {
        pitch.find_in(()).approx_value.as_piano_key()
    };

    let id = event.id;

    match event.phase {
        EventPhase::Pressed => {
            match model.synth_mode {
                SynthMode::Waveform => {
                    model.audio.start_waveform(
                        id,
                        pitch,
                        &model.waveforms[model.selected_waveform],
                    );
                }
                SynthMode::Fluid => {
                    model.audio.start_fluid_note(id, key.midi_number());
                }
            }
            model.pressed_keys.insert(id, VirtualKey { pitch });
        }
        EventPhase::Moved if model.legato => {
            model.audio.update_waveform(id, pitch);
            model.audio.update_fluid_note(&id, key.midi_number());
            if let Some(pressed_key) = model.pressed_keys.get_mut(&id) {
                pressed_key.pitch = pitch;
            }
        }
        EventPhase::Released => {
            model.audio.stop_waveform(id);
            model.audio.stop_fluid_note(&id);
            model.pressed_keys.remove(&id);
        }
        _ => {}
    }
}
