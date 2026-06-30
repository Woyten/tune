use std::sync::Arc;
use std::sync::Mutex;

use clap::Parser;
use flume::Receiver;
use tune::note::NoteLetter;
use tune::pitch::Pitch;
use tune::scala::Kbm;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::scala::SegmentType;
use tune::scala::create_harmonics_scale;
use tune::tuning::Tuning;

use crate::backend::Backend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::control::LiveParameter;
use crate::control::LiveParameterMapper;
use crate::control::LiveParameterStorage;
use crate::piano::PianoEngine;
use crate::piano::PressedKeys;
use crate::piano::SourceId;
use crate::piano::TuningMode;
use crate::profile::ColorPalette;
use crate::toggle::Direction;
use crate::tuning_layout::CustomKeyboardOptions;
use crate::tuning_layout::TuningLayout;

pub struct PianoEngineFixture {
    engine: PianoEngine,
    recorded_calls: Arc<Mutex<Vec<(usize, RecordedCall)>>>,
    storage_updates: Receiver<LiveParameterStorage>,
    latest_storage: LiveParameterStorage,
    expected: ExpectedOutput,
}

impl PianoEngineFixture {
    pub fn new() -> Self {
        let recorded_calls = Arc::new(Mutex::new(Vec::new()));
        let backends: Vec<Box<dyn Backend<SourceId>>> = [
            (NoteInput::Foreground, true),
            (NoteInput::Background, true),
            (NoteInput::Foreground, false),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (note_input, legato))| {
            Box::new(RecordingBackend::new(
                recorded_calls.clone(),
                index,
                note_input,
                legato,
            )) as Box<dyn Backend<SourceId>>
        })
        .collect();
        let tuning_layouts = vec![edo_12_tuning_layout(), harmonics_tuning_layout()].into();
        let mut mapper = LiveParameterMapper::new();
        mapper.push_mapping(LiveParameter::Legato, LEGATO_CONTROLLER);
        let storage = LiveParameterStorage::default();
        let (storage_updates_send, storage_updates_recv) = flume::unbounded();

        let engine = PianoEngine::new(
            backends,
            tuning_layouts,
            mapper,
            storage,
            storage_updates_send,
            None,
        );

        let mut fixture = Self {
            engine,
            recorded_calls,
            storage_updates: storage_updates_recv,
            latest_storage: LiveParameterStorage::default(),
            expected: ExpectedOutput::new(),
        };

        fixture.expect(|e| {
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
            e.expected_calls
                .push((BACKGROUND_LEGATO_BACKEND, RecordedCall::SetTuning));
            e.expected_calls
                .push((FOREGROUND_NO_LEGATO_BACKEND, RecordedCall::SetTuning));
            e.expected_calls
                .push((FOREGROUND_LEGATO_BACKEND, RecordedCall::RequestStatus));
        });

        fixture
    }

    pub fn when(&mut self, f: impl FnOnce(&PianoEngine)) -> &mut Self {
        f(&self.engine);
        while let Ok(storage) = self.storage_updates.try_recv() {
            self.latest_storage = storage;
        }
        self
    }

    pub fn expect(&mut self, f: impl FnOnce(&mut ExpectedOutput)) {
        f(&mut self.expected);

        let state = self.engine.capture_state();

        assert_eq!(
            state.tuning_mode, self.expected.tuning_mode,
            "tuning_mode mismatch"
        );

        assert_eq!(
            state.pressed_keys, self.expected.pressed_keys,
            "pressed_keys mismatch"
        );

        assert_eq!(
            state.keys_version, self.expected.keys_version,
            "keys_version mismatch"
        );

        assert_eq!(
            *self.recorded_calls.lock().unwrap(),
            self.expected.expected_calls,
            "recorded calls mismatch"
        );

        assert_eq!(
            self.latest_storage, self.expected.expected_storage,
            "storage mismatch"
        );
    }
}

pub struct RecordingBackend {
    calls: Arc<Mutex<Vec<(usize, RecordedCall)>>>,
    index: usize,
    note_input: NoteInput,
    legato: bool,
}

impl RecordingBackend {
    fn new(
        calls: Arc<Mutex<Vec<(usize, RecordedCall)>>>,
        index: usize,
        note_input: NoteInput,
        legato: bool,
    ) -> Self {
        Self {
            calls,
            index,
            note_input,
            legato,
        }
    }

    fn push_call(&self, call: RecordedCall) {
        self.calls.lock().unwrap().push((self.index, call));
    }
}

impl Backend<SourceId> for RecordingBackend {
    fn note_input(&self) -> NoteInput {
        self.note_input
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {
        self.push_call(RecordedCall::SetTuning);
    }

    fn set_no_tuning(&mut self) {
        self.push_call(RecordedCall::SetNoTuning);
    }

    fn request_status(&mut self) {
        self.push_call(RecordedCall::RequestStatus);
    }

    fn start(&mut self, key_id: SourceId, degree: i32, pitch: Pitch, velocity: u8) {
        self.push_call(RecordedCall::Start {
            key_id,
            degree,
            pitch,
            velocity,
        });
    }

    fn update_pitch(&mut self, key_id: SourceId, degree: i32, pitch: Pitch, velocity: u8) {
        self.push_call(RecordedCall::UpdatePitch {
            key_id,
            degree,
            pitch,
            velocity,
        });
    }

    fn update_pressure(&mut self, key_id: SourceId, pressure: u8) {
        self.push_call(RecordedCall::UpdatePressure { key_id, pressure });
    }

    fn stop(&mut self, key_id: SourceId, velocity: u8) {
        self.push_call(RecordedCall::Stop { key_id, velocity });
    }

    fn switch_bank(&mut self, _direction: Direction) {
        self.push_call(RecordedCall::SwitchBank);
    }

    fn program_change(&mut self, _program_change: ProgramChange) {
        self.push_call(RecordedCall::SwitchProgram);
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.push_call(RecordedCall::ControlChange { controller, value });
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.push_call(RecordedCall::ChannelPressure { pressure });
    }

    fn pitch_bend(&mut self, value: i16) {
        self.push_call(RecordedCall::PitchBend { value });
    }

    fn switch_envelope_type(&mut self, _direction: Direction) {
        self.push_call(RecordedCall::SwitchEnvelopeType);
    }

    fn has_legato(&self) -> bool {
        self.legato
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordedCall {
    Start {
        key_id: SourceId,
        degree: i32,
        pitch: Pitch,
        velocity: u8,
    },
    UpdatePitch {
        key_id: SourceId,
        degree: i32,
        pitch: Pitch,
        velocity: u8,
    },
    UpdatePressure {
        key_id: SourceId,
        pressure: u8,
    },
    Stop {
        key_id: SourceId,
        velocity: u8,
    },
    SetTuning,
    SetNoTuning,
    ControlChange {
        controller: u8,
        value: u8,
    },
    ChannelPressure {
        pressure: u8,
    },
    PitchBend {
        value: i16,
    },
    SwitchBank,
    SwitchProgram,
    SwitchEnvelopeType,
    RequestStatus,
}

pub struct ExpectedOutput {
    pub tuning_mode: TuningMode,
    pub pressed_keys: PressedKeys,
    pub keys_version: u64,
    pub expected_calls: Vec<(usize, RecordedCall)>,
    pub expected_storage: LiveParameterStorage,
}

impl ExpectedOutput {
    pub fn new() -> Self {
        Self {
            tuning_mode: TuningMode::Fixed,
            pressed_keys: PressedKeys::new(),
            keys_version: 0,
            expected_calls: Vec::new(),
            expected_storage: LiveParameterStorage::default(),
        }
    }
}

pub const FOREGROUND_LEGATO_BACKEND: usize = 0;
pub const BACKGROUND_LEGATO_BACKEND: usize = 1;
pub const FOREGROUND_NO_LEGATO_BACKEND: usize = 2;

// The actual source ID is irrelevant
pub const SRC_A: SourceId = SourceId::Touchpad(0);
pub const SRC_B: SourceId = SourceId::Touchpad(1);

pub const LEGATO_CONTROLLER: u8 = 68;

fn edo_12_scl() -> Scl {
    Scl::builder().push_cents(100.0).build().unwrap()
}

fn edo_12_kbm() -> Kbm {
    Kbm::builder(NoteLetter::D.in_octave(4)).build().unwrap()
}

pub fn edo_12_pitch(degree: i32) -> Pitch {
    let kbm_root = edo_12_kbm().kbm_root();
    let tuning = (&edo_12_scl(), kbm_root);
    tuning.pitch_of(degree)
}

fn edo_12_tuning_layout() -> TuningLayout {
    TuningLayout::new(
        edo_12_scl(),
        edo_12_kbm(),
        CustomKeyboardOptions::parse_from([""; 0]),
        &ColorPalette::default(),
    )
}

fn harmonics_scl() -> Scl {
    create_harmonics_scale(None, SegmentType::Otonal, 8, 8, None).unwrap()
}

fn harmonics_kbm() -> Kbm {
    Kbm::builder(NoteLetter::C.in_octave(4))
        .push_mapped_key(0)
        .push_unmapped_key()
        .push_mapped_key(1)
        .push_unmapped_key()
        .push_mapped_key(2)
        .push_unmapped_key()
        .push_mapped_key(3)
        .push_mapped_key(4)
        .push_mapped_key(5)
        .push_unmapped_key()
        .push_mapped_key(6)
        .push_mapped_key(7)
        .formal_octave(8)
        .build()
        .unwrap()
}

pub fn harmonics_pitch(degree: i32) -> Pitch {
    let kbm_root = harmonics_kbm().kbm_root();
    let tuning = (&harmonics_scl(), kbm_root);
    tuning.pitch_of(degree)
}

fn harmonics_tuning_layout() -> TuningLayout {
    TuningLayout::new(
        harmonics_scl(),
        harmonics_kbm(),
        CustomKeyboardOptions::parse_from([""; 0]),
        &ColorPalette::default(),
    )
}
