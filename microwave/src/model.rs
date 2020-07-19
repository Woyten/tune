use crate::{
    audio::Audio,
    midi::{ChannelMessage, ChannelMessageType},
    wave::{self, Patch},
};
use midir::MidiInputConnection;
use nannou::{
    event::{ElementState, KeyboardInput},
    prelude::*,
    winit::event::WindowEvent,
};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    mem,
    sync::{mpsc::Receiver, Arc, Mutex, MutexGuard},
};
use tune::{
    key::{Keyboard, PianoKey},
    key_map::KeyMap,
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    ratio::Ratio,
    scale::Scale,
    tuning::Tuning,
};

pub struct Model {
    pub engine: Arc<PianoEngine>,
    pub keyboard: Keyboard,
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

impl Model {
    pub fn new(
        engine: Arc<PianoEngine>,
        keyboard: Keyboard,
        midi_in: Option<MidiInputConnection<()>>,
        program_updates: Receiver<SelectedProgram>,
    ) -> Self {
        let lowest_note = NoteLetter::Fsh.in_octave(2).pitch();
        let highest_note = NoteLetter::Ash.in_octave(5).pitch();
        Self {
            engine,
            keyboard,
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

#[derive(Copy, Clone, Debug)]
pub enum SynthMode {
    OnlyWaveform,
    Waveform,
    Fluid,
}

#[derive(Copy, Clone, Debug)]
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
    Midi(u8),
}

enum EventPhase {
    Pressed(u8),
    Moved,
    Released,
}

pub fn event(_: &App, model: &mut Model, event: &WindowEvent) {
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
                model.virtual_keyboard_event(VirtualKeyboardEvent {
                    id: EventId::Keyboard(*scancode),
                    position: VirtualPosition::Key(key_number),
                    phase,
                });
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
    let engine = &model.engine;
    let mut engine_model = engine.model_write();
    match key {
        Key::L if app.keys.mods.ctrl() => engine_model.legato = !engine_model.legato,
        Key::Space => {
            engine_model.synth_mode = match engine_model.synth_mode {
                SynthMode::OnlyWaveform => SynthMode::OnlyWaveform,
                SynthMode::Waveform => SynthMode::Fluid,
                SynthMode::Fluid => SynthMode::Waveform,
            }
        }
        Key::Up => match engine_model.synth_mode {
            SynthMode::OnlyWaveform | SynthMode::Waveform => {
                engine_model.waveform_number =
                    (engine_model.waveform_number + engine.waveforms.len() - 1)
                        % engine.waveforms.len();
                engine_model.waveform_name = engine.waveforms[engine_model.waveform_number]
                    .name()
                    .to_owned()
            }
            SynthMode::Fluid => {
                model.selected_program.program_number =
                    (model.selected_program.program_number + 128 - 1) % 128;
                engine.set_program(model.selected_program.program_number);
            }
        },
        Key::Down => match engine_model.synth_mode {
            SynthMode::OnlyWaveform | SynthMode::Waveform => {
                engine_model.waveform_number =
                    (engine_model.waveform_number + 1) % engine.waveforms.len();
                engine_model.waveform_name = engine.waveforms[engine_model.waveform_number]
                    .name()
                    .to_owned()
            }
            SynthMode::Fluid => {
                model.selected_program.program_number =
                    (model.selected_program.program_number + 1) % 128;
                engine.set_program(model.selected_program.program_number);
            }
        },
        Key::Left => {
            engine_model.root_note = engine_model.root_note.plus_semitones(-1);
            mem::drop(engine_model); // Release the lock (will be fixed later)
            engine.retune();
        }
        Key::Right => {
            engine_model.root_note = engine_model.root_note.plus_semitones(1);
            mem::drop(engine_model); // Release the lock (will be fixed later)
            engine.retune();
        }
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
    let event = VirtualKeyboardEvent {
        id: EventId::Mouse,
        position: VirtualPosition::Coord(position),
        phase,
    };
    model.virtual_keyboard_event(event);
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
    let event = VirtualKeyboardEvent {
        id: EventId::Touchpad(event.id),
        position: VirtualPosition::Coord(position),
        phase,
    };
    model.virtual_keyboard_event(event);
}

pub fn update(_: &App, app_model: &mut Model, _: Update) {
    for update in app_model.program_updates.try_iter() {
        app_model.selected_program = update
    }
}

impl Model {
    fn virtual_keyboard_event(&mut self, event: VirtualKeyboardEvent) {
        let mut engine_model = self.engine.model_write();
        match event.position {
            VirtualPosition::Coord(position) => {
                let keyboard_range = Ratio::between_pitches(self.lowest_note, self.highest_note);

                let pitch = self.lowest_note
                    * Ratio::from_octaves(
                        keyboard_range.as_octaves() * Into::<f64>::into(position.x),
                    );

                if let Some(scale) = &engine_model.scale {
                    let key_map = KeyMap::root_at(engine_model.root_note);
                    let key = scale
                        .with_key_map(&key_map)
                        .find_by_pitch(pitch)
                        .approx_value;

                    let pitch = scale.with_key_map(&key_map).pitch_of(key);
                    self.engine
                        .process_event(&mut engine_model, event, key, pitch);
                } else {
                    let key = pitch.find_in(()).approx_value.as_piano_key();
                    self.engine
                        .process_event(&mut engine_model, event, key, pitch);
                }
            }
            VirtualPosition::Key(key) => {
                let key = engine_model.root_note.plus_semitones(key).as_piano_key();
                self.engine
                    .process_positional_event(&mut engine_model, event, key);
            }
        }
    }
}

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
    read_cache: Mutex<PianoEngineModel>,
    waveforms: Vec<Patch>,
    audio: Mutex<Audio<EventId>>,
}

impl PianoEngine {
    pub fn new(
        synth_mode: SynthMode,
        scale: Option<Scale>,
        program_number: u8,
        audio: Audio<EventId>,
    ) -> Self {
        let waveforms = wave::all_waveforms();

        let model = PianoEngineModel {
            synth_mode,
            scale,
            root_note: NoteLetter::D.in_octave(4),
            legato: true,
            waveform_number: 0,
            waveform_name: waveforms[0].name().to_owned(),
            pressed_keys: HashMap::new(),
        };

        let engine = Self {
            model: Mutex::new(model.clone()),
            read_cache: Mutex::new(model),
            waveforms,
            audio: Mutex::new(audio),
        };

        engine.retune();
        engine.set_program(program_number);

        engine
    }

    pub fn model_write(&self) -> MutexGuard<PianoEngineModel> {
        self.model.lock().unwrap()
    }

    pub fn model_read(&self) -> MutexGuard<PianoEngineModel> {
        let mut cached_model = self.read_cache.lock().unwrap();
        cached_model.clone_from(&self.model.lock().unwrap());
        cached_model
    }

    fn retune(&self) {
        let model = self.model_write();
        if let Some(scale) = &model.scale {
            self.audio
                .lock()
                .unwrap()
                .retune(scale.with_key_map(&KeyMap::root_at(model.root_note)))
        };
    }

    fn set_program(&self, program_number: u8) {
        self.audio
            .lock()
            .unwrap()
            .submit_fluid_message(ChannelMessage {
                channel: 0,
                message_type: ChannelMessageType::ProgramChange {
                    program: program_number,
                },
            });
    }

    pub fn process_midi_event(&self, message: ChannelMessage) {
        let event = match message.message_type {
            ChannelMessageType::NoteOn { key, velocity } => {
                Some((key, EventPhase::Pressed(velocity)))
            }
            ChannelMessageType::NoteOff { key, .. } => Some((key, EventPhase::Released)),
            _ => None,
        };
        if let Some((key, phase)) = event {
            let mut model = self.model_write();
            let event = VirtualKeyboardEvent {
                id: EventId::Midi(key),
                position: VirtualPosition::Key(
                    model
                        .root_note
                        .num_semitones_before(Note::from_midi_number(key.into())),
                ),
                phase,
            };
            self.process_positional_event(
                &mut model,
                event,
                PianoKey::from_midi_number(key.into()),
            );
        } else {
            self.audio.lock().unwrap().submit_fluid_message(message);
        }
    }

    fn process_positional_event(
        &self,
        model: &mut PianoEngineModel,
        event: VirtualKeyboardEvent,
        key: PianoKey,
    ) {
        let pitch = if let Some(scale) = &model.scale {
            let key_map = KeyMap::root_at(model.root_note);
            scale.with_key_map(&key_map).pitch_of(key)
        } else {
            Note::from_piano_key(key).pitch()
        };
        self.process_event(model, event, key, pitch);
    }

    fn process_event(
        &self,
        model: &mut PianoEngineModel,
        event: VirtualKeyboardEvent,
        key: PianoKey,
        pitch: Pitch,
    ) {
        let mut audio = self.audio.lock().unwrap();
        let id = event.id;

        match event.phase {
            EventPhase::Pressed(velocity) => {
                match model.synth_mode {
                    SynthMode::OnlyWaveform | SynthMode::Waveform => {
                        audio.start_waveform(id, pitch, &self.waveforms[model.waveform_number]);
                    }
                    SynthMode::Fluid => {
                        audio.start_fluid_note(id, key.midi_number(), velocity);
                    }
                }
                model.pressed_keys.insert(id, VirtualKey { pitch });
            }
            EventPhase::Moved if model.legato => {
                audio.update_waveform(id, pitch);
                audio.update_fluid_note(&id, key.midi_number(), 100);
                if let Some(pressed_key) = model.pressed_keys.get_mut(&id) {
                    pressed_key.pitch = pitch;
                }
            }
            EventPhase::Released => {
                audio.stop_waveform(id);
                audio.stop_fluid_note(&id);
                model.pressed_keys.remove(&id);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct PianoEngineModel {
    pub synth_mode: SynthMode,
    pub scale: Option<Scale>,
    pub root_note: Note,
    pub legato: bool,
    pub waveform_number: usize,
    pub waveform_name: String,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
}
