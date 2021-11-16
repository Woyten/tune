use std::{collections::HashMap, hash::Hash, iter};

use crate::{
    midi::{ChannelMessage, ChannelMessageType},
    mts::{
        self, ScaleOctaveTuning, ScaleOctaveTuningFormat, ScaleOctaveTuningMessage,
        ScaleOctaveTuningOptions, SingleNoteTuningChange, SingleNoteTuningChangeMessage,
        SingleNoteTuningChangeOptions,
    },
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched, Ratio},
};

use super::{AccessKeyResult, Group, JitTuner, PoolingMode, RegisterKeyResult};

pub struct JitMidiTuner<K, G, H> {
    handler: H,
    tuner: JitTuner<K, G>,
    first_channel: u8,
    midi_tuning_creator: MidiTuningCreator,
}

impl<K, H> JitMidiTuner<K, Note, H> {
    pub fn single_note_tuning_change(
        handler: H,
        first_channel: u8,
        num_channels: u8,
        pooling_mode: PoolingMode,
        device_id: u8,
        first_tuning_program: u8,
    ) -> Self {
        Self {
            handler,
            tuner: JitTuner::new(pooling_mode, usize::from(num_channels)),
            first_channel,
            midi_tuning_creator: MidiTuningCreator::SingleNoteTuningChange {
                device_id,
                first_tuning_program,
            },
        }
    }
}

impl<K, H> JitMidiTuner<K, NoteLetter, H> {
    pub fn scale_octave_tuning(
        handler: H,
        first_channel: u8,
        num_channels: u8,
        pooling_mode: PoolingMode,
        device_id: u8,
        format: ScaleOctaveTuningFormat,
    ) -> Self {
        Self {
            handler,
            tuner: JitTuner::new(pooling_mode, usize::from(num_channels)),
            first_channel,
            midi_tuning_creator: MidiTuningCreator::ScaleOctaveTuning {
                device_id,
                format,
                octave_tunings: HashMap::new(),
            },
        }
    }
}

impl<K, H> JitMidiTuner<K, (), H> {
    pub fn channel_fine_tuning(
        handler: H,
        first_channel: u8,
        num_channels: u8,
        pooling_mode: PoolingMode,
    ) -> Self {
        Self {
            handler,
            tuner: JitTuner::new(pooling_mode, usize::from(num_channels)),
            first_channel,
            midi_tuning_creator: MidiTuningCreator::ChannelFineTuning,
        }
    }

    pub fn pitch_bend(
        handler: H,
        first_channel: u8,
        num_channels: u8,
        pooling_mode: PoolingMode,
    ) -> Self {
        Self {
            handler,
            tuner: JitTuner::new(pooling_mode, usize::from(num_channels)),
            first_channel,
            midi_tuning_creator: MidiTuningCreator::PitchBend,
        }
    }
}

impl<K: Copy + Eq + Hash, G: Group + Copy + Eq + Hash, H: MidiTunerMessageHandler>
    JitMidiTuner<K, G, H>
{
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
                    self.send(
                        ChannelMessageType::NoteOff {
                            key: stopped_note,
                            velocity,
                        },
                        channel,
                    );
                }
                self.midi_tuning_creator.create(
                    channel,
                    self.first_channel,
                    started_note,
                    detuning,
                    &mut self.handler,
                );
                if let Some(started_note) = started_note.checked_midi_number() {
                    self.send(
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
                    self.send(
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
                self.midi_tuning_creator.create(
                    channel,
                    self.first_channel,
                    found_note,
                    detuning,
                    &mut self.handler,
                );
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
                    self.send(
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
        for channel in 0..self.tuner.num_channels() {
            self.send(message_type, channel);
        }
    }

    fn send(&mut self, message: ChannelMessageType, tuner_channel: usize) {
        let midi_channel =
            u8::try_from((usize::from(self.first_channel) + tuner_channel) % 16).unwrap();

        if let Some(channel_message) = message.in_channel(midi_channel) {
            self.handler.handle(MidiTunerMessage::new(channel_message));
        }
    }

    pub fn destroy(self) -> H {
        self.handler
    }
}

enum MidiTuningCreator {
    SingleNoteTuningChange {
        device_id: u8,
        first_tuning_program: u8,
    },
    ScaleOctaveTuning {
        device_id: u8,
        format: ScaleOctaveTuningFormat,
        octave_tunings: HashMap<usize, ScaleOctaveTuning>,
    },
    ChannelFineTuning,
    PitchBend,
}

impl MidiTuningCreator {
    fn create(
        &mut self,
        tuner_channel: usize,
        first_channel: u8,
        note: Note,
        detuning: Ratio,
        handler: &mut impl MidiTunerMessageHandler,
    ) {
        let midi_channel = u8::try_from((usize::from(first_channel) + tuner_channel) % 16).unwrap();

        match self {
            MidiTuningCreator::SingleNoteTuningChange {
                device_id,
                first_tuning_program,
            } => {
                let tuning_program =
                    u8::try_from((usize::from(*first_tuning_program) + tuner_channel) % 128)
                        .unwrap();

                if let Some(rpn_message) = mts::tuning_program_change(midi_channel, tuning_program)
                {
                    for channel_message in rpn_message {
                        handler.handle(MidiTunerMessage::new(channel_message));
                    }
                }

                let options = SingleNoteTuningChangeOptions {
                    device_id: *device_id,
                    tuning_program,
                    ..Default::default()
                };

                if let Ok(tuning_message) = SingleNoteTuningChangeMessage::from_tuning_changes(
                    &options,
                    iter::once(SingleNoteTuningChange {
                        key: note.as_piano_key(),
                        target_pitch: note.pitch() * detuning,
                    }),
                ) {
                    handler.handle(MidiTunerMessage::new(tuning_message));
                }
            }
            MidiTuningCreator::ScaleOctaveTuning {
                device_id,
                format,
                octave_tunings,
            } => {
                let octave_tuning = octave_tunings.entry(tuner_channel).or_default();
                *octave_tuning.as_mut(note.letter_and_octave().0) = detuning;
                let options = ScaleOctaveTuningOptions {
                    device_id: *device_id,
                    channels: midi_channel.into(),
                    format: *format,
                    ..Default::default()
                };
                if let Ok(tuning_message) =
                    ScaleOctaveTuningMessage::from_octave_tuning(&options, octave_tuning)
                {
                    handler.handle(MidiTunerMessage::new(tuning_message));
                }
            }
            MidiTuningCreator::ChannelFineTuning => {
                if let Some(rpn_message) = mts::channel_fine_tuning(midi_channel, detuning) {
                    for channel_message in rpn_message {
                        handler.handle(MidiTunerMessage::new(channel_message));
                    }
                }
            }
            MidiTuningCreator::PitchBend => {
                if let Some(channel_message) = (ChannelMessageType::PitchBendChange {
                    value: (detuning.as_semitones() / 2.0 * 8192.0) as i16,
                }
                .in_channel(midi_channel))
                {
                    handler.handle(MidiTunerMessage::new(channel_message));
                }
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
