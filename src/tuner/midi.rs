use std::{collections::HashMap, hash::Hash, iter};

use crate::{
    midi::{ChannelMessage, ChannelMessageType},
    mts::{
        self, ScaleOctaveTuning, ScaleOctaveTuningFormat, ScaleOctaveTuningMessage,
        ScaleOctaveTuningOptions, SingleNoteTuningChange, SingleNoteTuningChangeMessage,
        SingleNoteTuningChangeOptions,
    },
    note::Note,
    pitch::{Pitch, Pitched, Ratio},
    tuning::KeyboardMapping,
};

use super::{AccessKeyResult, AotTuner, GroupBy, JitTuner, PoolingMode, RegisterKeyResult};

pub struct AotMidiTuner<K, H> {
    target: MidiTarget<H>,
    tuner: AotTuner<K>,
    allow_pitch_pend: bool,
}

impl<K: Copy + Eq + Hash, H: MidiTunerMessageHandler> AotMidiTuner<K, H> {
    pub fn single_note_tuning_change(
        mut target: MidiTarget<H>,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
        realtime: bool,
        device_id: u8,
        first_tuning_program: u8,
    ) -> Result<Self, (MidiTarget<H>, usize)> {
        let (tuner, detunings) = AotTuner::apply_full_keyboard_tuning(tuning, keys);

        if detunings.len() > usize::from(target.num_channels) {
            return Err((target, detunings.len()));
        }

        for (tuner_channel, detuning) in detunings.iter().enumerate() {
            let midi_channel = target.midi_channel(tuner_channel);
            let tuning_program = target.tuning_program(tuner_channel, first_tuning_program);

            let options = SingleNoteTuningChangeOptions {
                realtime,
                device_id,
                tuning_program,
                with_bank_select: None,
            };

            if let Ok(tuning_message) = detuning.to_mts_format(&options) {
                for channel_message in
                    mts::tuning_program_change(midi_channel, tuning_program).unwrap()
                {
                    target
                        .handler
                        .handle(MidiTunerMessage::new(channel_message));
                }

                target.handler.handle(MidiTunerMessage::new(tuning_message));
            }
        }

        Ok(Self {
            target,
            tuner,
            allow_pitch_pend: true,
        })
    }

    pub fn scale_octave_tuning(
        mut target: MidiTarget<H>,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
        realtime: bool,
        device_id: u8,
        format: ScaleOctaveTuningFormat,
    ) -> Result<Self, (MidiTarget<H>, usize)> {
        let (tuner, detunings) = AotTuner::apply_octave_based_tuning(tuning, keys);

        if detunings.len() > usize::from(target.num_channels) {
            return Err((target, detunings.len()));
        }

        for (tuner_channel, detuning) in detunings.iter().enumerate() {
            let midi_channel = target.midi_channel(tuner_channel);

            let options = ScaleOctaveTuningOptions {
                realtime,
                device_id,
                channels: midi_channel.into(),
                format,
            };

            if let Ok(tuning_message) = detuning.to_mts_format(&options) {
                target.handler.handle(MidiTunerMessage::new(tuning_message));
            }
        }

        Ok(Self {
            target,
            tuner,
            allow_pitch_pend: true,
        })
    }

    pub fn channel_fine_tuning(
        mut target: MidiTarget<H>,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<Self, (MidiTarget<H>, usize)> {
        let (tuner, detunings) = AotTuner::apply_channel_based_tuning(tuning, keys);

        if detunings.len() > usize::from(target.num_channels) {
            return Err((target, detunings.len()));
        }

        for (tuner_channel, detuning) in detunings.iter().enumerate() {
            let midi_channel = target.midi_channel(tuner_channel);

            for channel_message in mts::channel_fine_tuning(midi_channel, *detuning).unwrap() {
                target
                    .handler
                    .handle(MidiTunerMessage::new(channel_message));
            }
        }

        Ok(Self {
            target,
            tuner,
            allow_pitch_pend: true,
        })
    }

    pub fn pitch_bend(
        mut target: MidiTarget<H>,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<Self, (MidiTarget<H>, usize)> {
        let (tuner, detunings) = AotTuner::apply_channel_based_tuning(tuning, keys);

        if detunings.len() > usize::from(target.num_channels) {
            return Err((target, detunings.len()));
        }

        for (tuner_channel, detuning) in detunings.iter().enumerate() {
            let midi_channel = target.midi_channel(tuner_channel);

            let channel_message = pitch_bend_message(*detuning)
                .in_channel(midi_channel)
                .unwrap();
            target
                .handler
                .handle(MidiTunerMessage::new(channel_message));
        }

        Ok(Self {
            target,
            tuner,
            allow_pitch_pend: false,
        })
    }

    pub fn note_on(&mut self, key: K, velocity: u8) {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(key) {
            if let Some(note) = note.checked_midi_number() {
                self.target.send(
                    ChannelMessageType::NoteOn {
                        key: note,
                        velocity,
                    },
                    channel,
                );
            }
        }
    }

    pub fn note_off(&mut self, key: K, velocity: u8) {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(key) {
            if let Some(note) = note.checked_midi_number() {
                self.target.send(
                    ChannelMessageType::NoteOff {
                        key: note,
                        velocity,
                    },
                    channel,
                );
            }
        }
    }

    pub fn key_pressure(&mut self, key: K, pressure: u8) {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(key) {
            if let Some(note) = note.checked_midi_number() {
                self.target.send(
                    ChannelMessageType::PolyphonicKeyPressure {
                        key: note,
                        pressure,
                    },
                    channel,
                );
            }
        }
    }

    pub fn send_monophonic_message(&mut self, message_type: ChannelMessageType) {
        self.target
            .send_monophonic_message(self.allow_pitch_pend, message_type);
    }

    pub fn destroy(self) -> MidiTarget<H> {
        self.target
    }
}

pub struct JitMidiTuner<K, H> {
    target: MidiTarget<H>,
    tuner: JitTuner<K>,
    midi_tuning_creator: MidiTuningCreator,
    allow_pitch_bend: bool,
}

impl<K: Copy + Eq + Hash, H: MidiTunerMessageHandler> JitMidiTuner<K, H> {
    pub fn single_note_tuning_change(
        target: MidiTarget<H>,
        pooling_mode: PoolingMode,
        realtime: bool,
        device_id: u8,
        first_tuning_program: u8,
    ) -> Self {
        Self {
            tuner: JitTuner::new(
                GroupBy::Note,
                pooling_mode,
                usize::from(target.num_channels),
            ),
            target,
            midi_tuning_creator: MidiTuningCreator::SingleNoteTuningChange {
                realtime,
                device_id,
                first_tuning_program,
            },
            allow_pitch_bend: true,
        }
    }

    pub fn scale_octave_tuning(
        target: MidiTarget<H>,
        pooling_mode: PoolingMode,
        realtime: bool,
        device_id: u8,
        format: ScaleOctaveTuningFormat,
    ) -> Self {
        Self {
            tuner: JitTuner::new(
                GroupBy::NoteLetter,
                pooling_mode,
                usize::from(target.num_channels),
            ),
            target,
            midi_tuning_creator: MidiTuningCreator::ScaleOctaveTuning {
                realtime,
                device_id,
                format,
                octave_tunings: HashMap::new(),
            },
            allow_pitch_bend: true,
        }
    }

    pub fn channel_fine_tuning(target: MidiTarget<H>, pooling_mode: PoolingMode) -> Self {
        Self {
            tuner: JitTuner::new(
                GroupBy::Channel,
                pooling_mode,
                usize::from(target.num_channels),
            ),
            target,
            midi_tuning_creator: MidiTuningCreator::ChannelFineTuning,
            allow_pitch_bend: true,
        }
    }

    pub fn pitch_bend(target: MidiTarget<H>, pooling_mode: PoolingMode) -> Self {
        Self {
            tuner: JitTuner::new(
                GroupBy::Channel,
                pooling_mode,
                usize::from(target.num_channels),
            ),
            target,
            midi_tuning_creator: MidiTuningCreator::PitchBend,
            allow_pitch_bend: false,
        }
    }

    /// Starts a note with the given `pitch`.
    ///
    /// `key` is used as identifier for currently sounding notes.
    pub fn note_on(&mut self, key: K, pitch: Pitch, velocity: u8) {
        match self.tuner.register_key(key, pitch) {
            RegisterKeyResult::Accepted {
                channel,
                stopped_note,
                started_note,
                detuning,
            } => {
                if let Some(stopped_note) = stopped_note.and_then(Note::checked_midi_number) {
                    self.target.send(
                        ChannelMessageType::NoteOff {
                            key: stopped_note,
                            velocity,
                        },
                        channel,
                    );
                }
                self.midi_tuning_creator
                    .create(&mut self.target, channel, started_note, detuning);
                if let Some(started_note) = started_note.checked_midi_number() {
                    self.target.send(
                        ChannelMessageType::NoteOn {
                            key: started_note,
                            velocity,
                        },
                        channel,
                    );
                }
            }
            RegisterKeyResult::Rejected => {}
        }
    }

    /// Stops the note of the given `key`.
    pub fn note_off(&mut self, key: &K, velocity: u8) {
        match self.tuner.deregister_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.target.send(
                        ChannelMessageType::NoteOff {
                            key: found_note,
                            velocity,
                        },
                        channel,
                    );
                }
            }
            AccessKeyResult::NotFound => {}
        }
    }

    /// Updates the note of `key` with the given `pitch`.
    pub fn update_pitch(&mut self, key: &K, pitch: Pitch) {
        match self.tuner.access_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                let detuning = Ratio::between_pitches(found_note.pitch(), pitch);
                self.midi_tuning_creator
                    .create(&mut self.target, channel, found_note, detuning);
            }
            AccessKeyResult::NotFound => {}
        }
    }

    /// Sends a key-pressure message to the note with the given `key`.
    pub fn key_pressure(&mut self, key: &K, pressure: u8) {
        match self.tuner.access_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.target.send(
                        ChannelMessageType::PolyphonicKeyPressure {
                            key: found_note,
                            pressure,
                        },
                        channel,
                    );
                }
            }
            AccessKeyResult::NotFound => {}
        }
    }

    /// Dispatches a channel-global message to all real MIDI channels.
    pub fn send_monophonic_message(&mut self, message_type: ChannelMessageType) {
        self.target
            .send_monophonic_message(self.allow_pitch_bend, message_type);
    }

    pub fn destroy(self) -> MidiTarget<H> {
        self.target
    }
}

pub struct MidiTarget<H> {
    pub handler: H,
    pub first_channel: u8,
    pub num_channels: u8,
}

impl<H: MidiTunerMessageHandler> MidiTarget<H> {
    fn send_monophonic_message(
        &mut self,
        allow_pitch_bend: bool,
        message_type: ChannelMessageType,
    ) {
        for channel in 0..self.num_channels {
            if allow_pitch_bend
                || !matches!(message_type, ChannelMessageType::PitchBendChange { .. })
            {
                self.send(message_type, usize::from(channel));
            }
        }
    }

    fn send(&mut self, message: ChannelMessageType, tuner_channel: usize) {
        if let Some(channel_message) = message.in_channel(self.midi_channel(tuner_channel)) {
            self.handler.handle(MidiTunerMessage::new(channel_message));
        }
    }

    fn midi_channel(&self, tuner_channel: usize) -> u8 {
        (u8::try_from(tuner_channel).unwrap() + self.first_channel) % 16
    }

    fn tuning_program(&self, tuner_channel: usize, first_tuning_program: u8) -> u8 {
        (u8::try_from(tuner_channel).unwrap() + first_tuning_program) % 128
    }
}

enum MidiTuningCreator {
    SingleNoteTuningChange {
        device_id: u8,
        realtime: bool,
        first_tuning_program: u8,
    },
    ScaleOctaveTuning {
        device_id: u8,
        realtime: bool,
        format: ScaleOctaveTuningFormat,
        octave_tunings: HashMap<usize, ScaleOctaveTuning>,
    },
    ChannelFineTuning,
    PitchBend,
}

impl MidiTuningCreator {
    fn create(
        &mut self,
        target: &mut MidiTarget<impl MidiTunerMessageHandler>,
        tuner_channel: usize,
        note: Note,
        detuning: Ratio,
    ) {
        let midi_channel = target.midi_channel(tuner_channel);

        match self {
            MidiTuningCreator::SingleNoteTuningChange {
                realtime,
                device_id,
                first_tuning_program,
            } => {
                let tuning_program = target.tuning_program(tuner_channel, *first_tuning_program);

                let options = SingleNoteTuningChangeOptions {
                    realtime: *realtime,
                    device_id: *device_id,
                    tuning_program,
                    with_bank_select: None,
                };

                if let Ok(tuning_message) = SingleNoteTuningChangeMessage::from_tuning_changes(
                    &options,
                    iter::once(SingleNoteTuningChange {
                        key: note.as_piano_key(),
                        target_pitch: note.pitch() * detuning,
                    }),
                ) {
                    for channel_message in
                        mts::tuning_program_change(midi_channel, tuning_program).unwrap()
                    {
                        target
                            .handler
                            .handle(MidiTunerMessage::new(channel_message));
                    }

                    target.handler.handle(MidiTunerMessage::new(tuning_message));
                }
            }
            MidiTuningCreator::ScaleOctaveTuning {
                realtime,
                device_id,
                format,
                octave_tunings,
            } => {
                let octave_tuning = octave_tunings.entry(tuner_channel).or_default();
                *octave_tuning.as_mut(note.letter_and_octave().0) = detuning;

                let options = ScaleOctaveTuningOptions {
                    realtime: *realtime,
                    device_id: *device_id,
                    channels: midi_channel.into(),
                    format: *format,
                };

                if let Ok(tuning_message) =
                    ScaleOctaveTuningMessage::from_octave_tuning(&options, octave_tuning)
                {
                    target.handler.handle(MidiTunerMessage::new(tuning_message));
                }
            }
            MidiTuningCreator::ChannelFineTuning => {
                for channel_message in mts::channel_fine_tuning(midi_channel, detuning).unwrap() {
                    target
                        .handler
                        .handle(MidiTunerMessage::new(channel_message));
                }
            }
            MidiTuningCreator::PitchBend => {
                let channel_message = pitch_bend_message(detuning)
                    .in_channel(midi_channel)
                    .unwrap();
                target
                    .handler
                    .handle(MidiTunerMessage::new(channel_message));
            }
        }
    }
}

pub struct MidiTunerMessage {
    variant: MidiTunerMessageVariant,
}

impl MidiTunerMessage {
    fn new<M: Into<MidiTunerMessageVariant>>(variant: M) -> Self {
        Self {
            variant: variant.into(),
        }
    }

    pub fn send_to(&self, mut receiver: impl FnMut(&[u8])) {
        match &self.variant {
            MidiTunerMessageVariant::Channel(channel_message) => {
                receiver(&channel_message.to_raw_message());
            }
            MidiTunerMessageVariant::ScaleOctaveTuning(tuning_message) => {
                receiver(tuning_message.sysex_bytes());
            }
            MidiTunerMessageVariant::SingleNoteTuningChange(tuning_message) => {
                for sysex_bytes in tuning_message.sysex_bytes() {
                    receiver(sysex_bytes);
                }
            }
        }
    }
}

enum MidiTunerMessageVariant {
    Channel(ChannelMessage),
    ScaleOctaveTuning(ScaleOctaveTuningMessage),
    SingleNoteTuningChange(SingleNoteTuningChangeMessage),
}

impl From<ChannelMessage> for MidiTunerMessageVariant {
    fn from(v: ChannelMessage) -> Self {
        Self::Channel(v)
    }
}

impl From<ScaleOctaveTuningMessage> for MidiTunerMessageVariant {
    fn from(v: ScaleOctaveTuningMessage) -> Self {
        Self::ScaleOctaveTuning(v)
    }
}

impl From<SingleNoteTuningChangeMessage> for MidiTunerMessageVariant {
    fn from(v: SingleNoteTuningChangeMessage) -> Self {
        Self::SingleNoteTuningChange(v)
    }
}

pub trait MidiTunerMessageHandler {
    fn handle(&mut self, message: MidiTunerMessage);
}

impl<F: FnMut(MidiTunerMessage)> MidiTunerMessageHandler for F {
    fn handle(&mut self, message: MidiTunerMessage) {
        self(message)
    }
}

fn pitch_bend_message(detuning: Ratio) -> ChannelMessageType {
    ChannelMessageType::PitchBendChange {
        value: (detuning.as_semitones() / 2.0 * 8192.0) as i16,
    }
}
