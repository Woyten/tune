use crate::{
    audio::Audio,
    wave::{self, Patch},
};
use nannou::{
    event::{ElementState, KeyboardInput},
    prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tune::{
    key::Keyboard,
    key_map::KeyMap,
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    ratio::Ratio,
    scale::Scale,
    tuning::Tuning,
};
use winit::event::WindowEvent;

pub struct Model {
    pub synth_mode: SynthMode,
    pub soundfont_provided: bool,
    pub scale: Option<Scale>,
    pub keyboard: Keyboard,
    pub root_note: Note,
    pub legato: bool,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pub waveforms: Vec<Patch>,
    pub selected_waveform: usize,
    pub program_number: u32,
    pub program_name: Arc<Mutex<Option<String>>>,
    pub pressed_physical_keys: HashSet<u32>,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
    pub audio: Audio<EventId>,
}

impl Model {
    pub fn new(
        scale: Option<Scale>,
        keyboard: Keyboard,
        soundfont_file_location: Option<PathBuf>,
        initial_program_number: u32,
    ) -> Self {
        let mut model = Self {
            synth_mode: if soundfont_file_location.is_some() {
                SynthMode::Fluid
            } else {
                SynthMode::Waveform
            },
            soundfont_provided: soundfont_file_location.is_some(),
            scale,
            keyboard,
            root_note: NoteLetter::D.in_octave(4),
            legato: true,
            lowest_note: NoteLetter::Fsh.in_octave(2).pitch(),
            highest_note: NoteLetter::Ash.in_octave(5).pitch(),
            waveforms: wave::all_waveforms(),
            selected_waveform: 0,
            program_number: initial_program_number,
            program_name: Arc::new(Mutex::new(None)),
            pressed_physical_keys: HashSet::new(),
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
    position: VirtualPosition,
    phase: EventPhase,
}

enum VirtualPosition {
    Coord(Point2),
    Key(i32),
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub enum EventId {
    Mouse,
    Touchpad(u64),
    Keyboard(u32),
}

enum EventPhase {
    Pressed,
    Moved,
    Released,
}

pub fn event(app: &App, model: &mut Model, event: &WindowEvent) {
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
                    EventPhase::Pressed,
                    model.pressed_physical_keys.insert(*scancode),
                ),
                ElementState::Released => (
                    EventPhase::Released,
                    model.pressed_physical_keys.remove(scancode),
                ),
            };

            // While a key is held down ElementState::Pressed is sent repeatedly. We ignore this case by checking net_change
            if net_change {
                virtual_keyboard(
                    app,
                    model,
                    VirtualKeyboardEvent {
                        id: EventId::Keyboard(*scancode),
                        position: VirtualPosition::Key(key_number),
                        phase,
                    },
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
    match key {
        Key::L if app.keys.mods.ctrl() => model.legato = !model.legato,
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

pub fn mouse_pressed(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        mouse_event(app, model, EventPhase::Pressed, app.mouse.position());
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

fn mouse_event(app: &App, model: &mut Model, phase: EventPhase, position: Point2) {
    let event = VirtualKeyboardEvent {
        id: EventId::Mouse,
        position: VirtualPosition::Coord(position),
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
        position: VirtualPosition::Coord(event.position),
        phase,
    };
    virtual_keyboard(app, model, event);
}

fn virtual_keyboard(app: &App, model: &mut Model, event: VirtualKeyboardEvent) {
    let (key, pitch) = match event.position {
        VirtualPosition::Coord(position) => {
            let x_position = position.x as f64 / app.window_rect().w() as f64 + 0.5;

            let keyboard_range = Ratio::between_pitches(model.lowest_note, model.highest_note);

            let pitch =
                model.lowest_note * Ratio::from_octaves(keyboard_range.as_octaves() * x_position);

            if let Some(scale) = &model.scale {
                let key_map = KeyMap::root_at(model.root_note);
                let key = scale
                    .with_key_map(&key_map)
                    .find_by_pitch(pitch)
                    .approx_value;
                (key, scale.with_key_map(&key_map).pitch_of(key))
            } else {
                (pitch.find_in(()).approx_value.as_piano_key(), pitch)
            }
        }
        VirtualPosition::Key(key) => {
            let key = model.root_note.plus_semitones(key).as_piano_key();
            if let Some(scale) = &model.scale {
                let key_map = KeyMap::root_at(model.root_note);
                (key, scale.with_key_map(&key_map).pitch_of(key))
            } else {
                (key, Note::from_piano_key(key).pitch())
            }
        }
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
