use std::collections::HashMap;

use crate::midi::ChannelMessage;
use crate::midi::ChannelMessageType;
use crate::mts;
use crate::mts::ScaleOctaveTuning;
use crate::mts::ScaleOctaveTuningFormat;
use crate::mts::ScaleOctaveTuningMessage;
use crate::mts::ScaleOctaveTuningOptions;
use crate::mts::SingleNoteTuningChange;
use crate::mts::SingleNoteTuningChangeMessage;
use crate::mts::SingleNoteTuningChangeOptions;
use crate::note::Note;
use crate::pitch::Pitched;
use crate::pitch::Ratio;
use crate::tuner::GroupBy;
use crate::tuner::TunableSynth;

pub struct TunableMidi<H> {
    midi_target: MidiTarget<H>,
    midi_tuning_creator: MidiTuningCreator,
}

impl<H> TunableMidi<H> {
    pub fn single_note_tuning_change(
        midi_target: MidiTarget<H>,
        realtime: bool,
        device_id: u8,
        first_tuning_program: u8,
    ) -> Self {
        Self {
            midi_target,
            midi_tuning_creator: MidiTuningCreator::SingleNoteTuningChange {
                realtime,
                device_id,
                first_tuning_program,
            },
        }
    }

    pub fn scale_octave_tuning(
        midi_target: MidiTarget<H>,
        realtime: bool,
        device_id: u8,
        format: ScaleOctaveTuningFormat,
    ) -> Self {
        Self {
            midi_target,
            midi_tuning_creator: MidiTuningCreator::ScaleOctaveTuning {
                realtime,
                device_id,
                format,
                octave_tunings: HashMap::new(),
            },
        }
    }

    pub fn channel_fine_tuning(midi_target: MidiTarget<H>) -> Self {
        Self {
            midi_target,
            midi_tuning_creator: MidiTuningCreator::ChannelFineTuning,
        }
    }

    pub fn pitch_bend(midi_target: MidiTarget<H>) -> Self {
        Self {
            midi_target,
            midi_tuning_creator: MidiTuningCreator::PitchBend,
        }
    }
}

impl<H: MidiTunerMessageHandler> TunableSynth for TunableMidi<H> {
    type Result = ();
    type NoteAttr = u8;
    type GlobalAttr = ChannelMessageType;

    fn num_channels(&self) -> usize {
        self.midi_target.channels.len()
    }

    fn group_by(&self) -> GroupBy {
        self.midi_tuning_creator.group_by()
    }

    fn notes_detune(&mut self, channel: usize, detuned_notes: &[(Note, Ratio)]) {
        self.midi_tuning_creator
            .create(&mut self.midi_target, channel, detuned_notes)
    }

    fn note_on(&mut self, channel: usize, started_note: Note, velocity: u8) {
        if let Some(started_note) = started_note.checked_midi_number() {
            self.midi_target.send(
                ChannelMessageType::NoteOn {
                    key: started_note,
                    velocity,
                },
                channel,
            );
        }
    }

    fn note_off(&mut self, channel: usize, stopped_note: Note, velocity: u8) {
        if let Some(stopped_note) = stopped_note.checked_midi_number() {
            self.midi_target.send(
                ChannelMessageType::NoteOff {
                    key: stopped_note,
                    velocity,
                },
                channel,
            );
        }
    }

    fn note_attr(&mut self, channel: usize, affected_note: Note, pressure: u8) {
        if let Some(affected_note) = affected_note.checked_midi_number() {
            self.midi_target.send(
                ChannelMessageType::PolyphonicKeyPressure {
                    key: affected_note,
                    pressure,
                },
                channel,
            );
        }
    }

    fn global_attr(&mut self, message_type: ChannelMessageType) {
        for channel in 0..self.num_channels() {
            if self.midi_tuning_creator.allow_pitch_bend()
                || !matches!(message_type, ChannelMessageType::PitchBendChange { .. })
            {
                self.midi_target.send(message_type, channel);
            }
        }
    }
}

pub struct MidiTarget<H> {
    pub handler: H,
    pub channels: Vec<u8>,
}

impl<H: MidiTunerMessageHandler> MidiTarget<H> {
    fn send(&mut self, message: ChannelMessageType, tuner_channel: usize) {
        self.handler
            .handle_channel_message(message, self.midi_channel(tuner_channel));
    }

    fn midi_channel(&self, tuner_channel: usize) -> u8 {
        self.channels[tuner_channel]
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
        detuned_notes: &[(Note, Ratio)],
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

                for channel_message in
                    mts::tuning_program_change(midi_channel, tuning_program).unwrap()
                {
                    target
                        .handler
                        .handle(MidiTunerMessage::new(channel_message));
                }

                if let Ok(tuning_message) = SingleNoteTuningChangeMessage::from_tuning_changes(
                    &options,
                    detuned_notes
                        .iter()
                        .map(|&(note, detuning)| SingleNoteTuningChange {
                            key: note.as_piano_key(),
                            target_pitch: note.pitch() * detuning,
                        }),
                ) {
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

                for &(note, detuning) in detuned_notes {
                    *octave_tuning.as_mut(note.letter_and_octave().0) = detuning;
                }

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
                for &(_, detuning) in detuned_notes {
                    for channel_message in mts::channel_fine_tuning(midi_channel, detuning).unwrap()
                    {
                        target
                            .handler
                            .handle(MidiTunerMessage::new(channel_message));
                    }
                }
            }
            MidiTuningCreator::PitchBend => {
                for &(_, detuning) in detuned_notes {
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

    fn group_by(&self) -> GroupBy {
        match self {
            MidiTuningCreator::SingleNoteTuningChange { .. } => GroupBy::Note,
            MidiTuningCreator::ScaleOctaveTuning { .. } => GroupBy::NoteLetter,
            MidiTuningCreator::ChannelFineTuning | MidiTuningCreator::PitchBend => GroupBy::Channel,
        }
    }

    fn allow_pitch_bend(&self) -> bool {
        match self {
            MidiTuningCreator::SingleNoteTuningChange { .. }
            | MidiTuningCreator::ScaleOctaveTuning { .. }
            | MidiTuningCreator::ChannelFineTuning => true,
            MidiTuningCreator::PitchBend => false,
        }
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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

    fn handle_channel_message(&mut self, message_type: ChannelMessageType, channel: u8) {
        if let Some(message) = message_type.in_channel(channel) {
            self.handle(MidiTunerMessage::new(message));
        }
    }
}

impl<F: FnMut(MidiTunerMessage)> MidiTunerMessageHandler for F {
    fn handle(&mut self, message: MidiTunerMessage) {
        self(message)
    }
}

fn pitch_bend_message(detuning: Ratio) -> ChannelMessageType {
    ChannelMessageType::PitchBendChange {
        value: ((detuning.as_semitones() / 2.0 * 8192.0) as i16)
            .max(-8192)
            .min(8192),
    }
}
