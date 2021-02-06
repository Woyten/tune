//! Communication with devices over the MIDI Tuning Standard.
//!
//! References:
//! - [Sysex messages](https://www.midi.org/specifications/item/table-4-universal-system-exclusive-messages)
//! - [MIDI Tuning Standard](http://www.microtonal-synthesis.com/MIDItuning.html)

use std::{collections::HashSet, convert::TryInto, fmt::Debug, iter};

use crate::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    note::{Note, NoteLetter},
    pitch::{Pitched, Ratio},
    tuning::Tuning,
};

// Universal System Exclusive Messages
// f0 7e <payload> f7 Non-Real Time
// f0 7f <payload> f7 Real Time

const SYSEX_START: u8 = 0xf0;
const SYSEX_END: u8 = 0xf7;

const SYSEX_RT: u8 = 0x7f;
const SYSEX_NON_RT: u8 = 0x7e;

// MIDI Tuning Standard (Non-Real Time)
// 08 00 Bulk Dump Request
// 08 01 Bulk Dump Reply
// 08 03 Tuning Dump Request
// 08 04 Key-Based Tuning Dump
// 08 05 Scale/Octave Tuning Dump, 1 byte format
// 08 06 Scale/Octave Tuning Dump, 2 byte format
// 08 07 Single Note Tuning Change with Bank Select
// 08 08 Scale/Octave Tuning, 1 byte format
// 08 09 Scale/Octave Tuning, 2 byte format

// MIDI Tuning Standard (Real Time)
// 08 02 Single Note Tuning Change
// 08 07 Single Note Tuning Change with Bank Select
// 08 08 Scale/Octave Tuning, 1 byte format
// 08 09 Scale/Octave Tuning, 2 byte format

const MIDI_TUNING_STANDARD: u8 = 0x08;

const SINGLE_NOTE_TUNING_CHANGE: u8 = 0x02;
const SCALE_OCTAVE_TUNING_1_BYTE_FORMAT: u8 = 0x08;

const DEVICE_ID_BROADCAST: u8 = 0x7f;

const U7_MASK: u16 = (1 << 7) - 1;
const U14_UPPER_BOUND_AS_F64: f64 = (1 << 14) as f64;

#[derive(Clone, Debug)]
pub struct SingleNoteTuningChangeMessage {
    sysex_calls: Vec<Vec<u8>>,
    retuned_notes: Vec<SingleNoteTuningChange>,
    out_of_range_notes: Vec<SingleNoteTuningChange>,
}

impl SingleNoteTuningChangeMessage {
    pub fn from_tuning(
        tuning: impl Tuning<PianoKey>,
        keys: impl IntoIterator<Item = PianoKey>,
        device_id: DeviceId,
        tuning_program: u8,
    ) -> Result<Self, SingleNoteTuningChangeError> {
        let tuning_changes = keys
            .into_iter()
            .map(|key| SingleNoteTuningChange::new(key, tuning.pitch_of(key)));
        Self::from_tuning_changes(tuning_changes, device_id, tuning_program)
    }

    /// Creates a [`SingleNoteTuningChangeMessage`] from the provided `tuning_changes`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::iter::FromIterator;
    /// # use tune::mts::SingleNoteTuningChange;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// let a4 = NoteLetter::A.in_octave(4).as_piano_key();
    /// let target_pitch = Pitch::from_hz(445.0);
    ///
    /// let tuning_changes = std::iter::once(SingleNoteTuningChange::new(a4, target_pitch));
    /// let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
    ///     tuning_changes,
    ///     Default::default(),
    ///     55,
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(
    ///     Vec::from_iter(tuning_message.sysex_bytes()),
    ///     [[0xf0, 0x7f, 0x7f, 0x08, 0x02, 55, 1, 69, 69, 25, 5, 0xf7]]
    /// );
    /// ```
    pub fn from_tuning_changes(
        tuning_changes: impl IntoIterator<Item = SingleNoteTuningChange>,
        device_id: DeviceId,
        tuning_program: u8,
    ) -> Result<Self, SingleNoteTuningChangeError> {
        if tuning_program >= 128 {
            return Err(SingleNoteTuningChangeError::TuningProgramOutOfRange);
        }

        let mut sysex_tuning_list = Vec::new();
        let mut retuned_notes = Vec::new();
        let mut out_of_range_notes = Vec::new();

        for (number_of_notes, tuning) in tuning_changes.into_iter().enumerate() {
            if number_of_notes >= 128 {
                return Err(SingleNoteTuningChangeError::TuningChangeListTooLong);
            }

            if let (Some(source), Some(target)) = (
                tuning.key.checked_midi_number(),
                tuning.target_note.checked_midi_number(),
            ) {
                let pitch_msb = (tuning.detune_as_u14 >> 7) as u8;
                let pitch_lsb = (tuning.detune_as_u14 & U7_MASK) as u8;

                sysex_tuning_list.push(source);
                sysex_tuning_list.push(target);
                sysex_tuning_list.push(pitch_msb);
                sysex_tuning_list.push(pitch_lsb);

                retuned_notes.push(tuning);
            } else {
                out_of_range_notes.push(tuning);
            }
        }

        fn create_sysex(
            device_id: DeviceId,
            tuning_program: u8,
            sysex_tuning_list: &[u8],
        ) -> Vec<u8> {
            let mut sysex_call = Vec::new();
            sysex_call.push(SYSEX_START);
            sysex_call.push(SYSEX_RT);
            sysex_call.push(device_id.as_u8());
            sysex_call.push(MIDI_TUNING_STANDARD);
            sysex_call.push(SINGLE_NOTE_TUNING_CHANGE);
            sysex_call.push(tuning_program);
            sysex_call.push((sysex_tuning_list.len() / 4).try_into().unwrap());
            sysex_call.extend(sysex_tuning_list);
            sysex_call.push(SYSEX_END);

            sysex_call
        }

        let mut sysex_calls = Vec::new();
        if retuned_notes.len() == 128 {
            sysex_calls.push(create_sysex(
                device_id,
                tuning_program,
                &sysex_tuning_list[..256],
            ));
            sysex_calls.push(create_sysex(
                device_id,
                tuning_program,
                &sysex_tuning_list[256..],
            ));
        } else {
            sysex_calls.push(create_sysex(
                device_id,
                tuning_program,
                &sysex_tuning_list[..],
            ));
        }

        Ok(SingleNoteTuningChangeMessage {
            sysex_calls,
            retuned_notes,
            out_of_range_notes,
        })
    }

    /// Returns the tuning message conforming to the MIDI tuning standard.
    ///
    /// If less than 128 notes are retuned the iterator yields a single tuning message.
    /// If the number of retuned notes is 128 two messages with a batch of 64 notes are yielded.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::iter::FromIterator;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::key::PianoKey;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::KbmRoot;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_ratio(Ratio::octave().divided_into_equal_steps(31))
    ///     .build()
    ///     .unwrap();
    /// let kbm = KbmRoot::from(NoteLetter::D.in_octave(4));
    /// let tuning = (scl, kbm);
    ///
    /// let single_message = SingleNoteTuningChangeMessage::from_tuning(
    ///     &tuning,
    ///     (0..127).map(PianoKey::from_midi_number),
    ///     Default::default(),
    ///     0,
    /// )
    /// .unwrap();
    /// assert_eq!(Vec::from_iter(single_message.sysex_bytes()).len(), 1);
    ///
    /// let split_message = SingleNoteTuningChangeMessage::from_tuning(
    ///     &tuning,
    ///     (0..128).map(PianoKey::from_midi_number),
    ///     Default::default(),
    ///     0,
    /// )
    /// .unwrap();
    /// assert_eq!(Vec::from_iter(split_message.sysex_bytes()).len(), 2);
    /// ```
    pub fn sysex_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.sysex_calls.iter().map(Vec::as_slice)
    }

    pub fn retuned_notes(&self) -> &[SingleNoteTuningChange] {
        &self.retuned_notes
    }

    pub fn out_of_range_notes(&self) -> &[SingleNoteTuningChange] {
        &self.out_of_range_notes
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SingleNoteTuningChange {
    key: PianoKey,
    target_note: Note,
    detune_as_u14: u16,
}

#[derive(Copy, Clone, Debug)]
pub enum SingleNoteTuningChangeError {
    /// The tuning change list has more than 128 elements.
    TuningChangeListTooLong,

    /// The tuning program number is higher than 128.
    TuningProgramOutOfRange,
}

impl SingleNoteTuningChange {
    pub fn new(key: PianoKey, pitched: impl Pitched) -> Self {
        let approximation = pitched.pitch().find_in_tuning(());

        let mut target_note = approximation.approx_value;
        let mut detune_in_u14_resolution =
            (approximation.deviation.as_semitones() * U14_UPPER_BOUND_AS_F64).round();

        // Make sure that the detune range is [0c..100c] instead of [-50c..50c]
        if detune_in_u14_resolution < 0.0 {
            target_note = target_note.plus_semitones(-1);
            detune_in_u14_resolution += U14_UPPER_BOUND_AS_F64;
        }

        Self {
            key,
            target_note,
            detune_as_u14: detune_in_u14_resolution as u16,
        }
    }
}

pub struct ScaleOctaveTuningMessage {
    sysex_call: Vec<u8>,
}

impl ScaleOctaveTuningMessage {
    pub fn from_scale_octave_tuning(
        octave_tuning: &ScaleOctaveTuning,
        channels: impl Into<Channels>,
        device_id: DeviceId,
    ) -> Result<Self, ScaleOctaveTuningError> {
        let mut sysex_call = Vec::new();

        sysex_call.push(SYSEX_START);
        sysex_call.push(SYSEX_NON_RT);
        sysex_call.push(device_id.as_u8());
        sysex_call.push(MIDI_TUNING_STANDARD);
        sysex_call.push(SCALE_OCTAVE_TUNING_1_BYTE_FORMAT);

        match channels.into() {
            Channels::All => {
                sysex_call.push(0b0000_0011); // bits 0 to 1 = channel 15 to 16
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 8 to 14
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 1 to 7
            }
            Channels::Some(channels) => {
                let mut encoded_channels = [0; 3];

                for channel in channels {
                    if channel >= 16 {
                        return Err(ScaleOctaveTuningError::ChannelOutOfRange);
                    }
                    let bit_position = channel % 7;
                    let row_to_use = channel / 7;
                    encoded_channels[usize::from(row_to_use)] |= 1 << bit_position;
                }

                sysex_call.extend(encoded_channels.iter().rev());
            }
        }
        for &pitch_bend in [
            octave_tuning.c,
            octave_tuning.csh,
            octave_tuning.d,
            octave_tuning.dsh,
            octave_tuning.e,
            octave_tuning.f,
            octave_tuning.fsh,
            octave_tuning.g,
            octave_tuning.gsh,
            octave_tuning.a,
            octave_tuning.ash,
            octave_tuning.b,
        ]
        .iter()
        {
            let cents_value = pitch_bend.as_cents().round() + 64.0;
            if !(0.0..=127.0).contains(&cents_value) {
                return Err(ScaleOctaveTuningError::DetuningOutOfRange);
            }
            sysex_call.push(cents_value as u8);
        }
        sysex_call.push(SYSEX_END);

        Ok(ScaleOctaveTuningMessage { sysex_call })
    }

    pub fn sysex_bytes(&self) -> &[u8] {
        &self.sysex_call
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScaleOctaveTuningError {
    /// The tuning of a note exceeds the allowed range [-64..=63] cents.
    DetuningOutOfRange,

    /// A channel number exceeds the allowed range [0..16).
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::collections::HashSet;
    /// # use std::iter::FromIterator;
    /// # use tune::mts::ScaleOctaveTuningMessage;
    /// # use tune::mts::ScaleOctaveTuningError;
    /// let only_valid_channels = HashSet::from_iter([14, 15].iter().copied());
    /// assert!(matches!(
    ///     ScaleOctaveTuningMessage::from_scale_octave_tuning(
    ///         &Default::default(), only_valid_channels, Default::default(),
    ///     ),
    ///     Ok(_)
    /// ));
    ///
    /// let channel_16_is_invalid = HashSet::from_iter([14, 15, 16].iter().copied());
    /// assert!(matches!(
    ///     ScaleOctaveTuningMessage::from_scale_octave_tuning(
    ///         &Default::default(), channel_16_is_invalid, Default::default(),
    ///     ),
    ///     Err(ScaleOctaveTuningError::ChannelOutOfRange)
    /// ));
    /// ```
    ChannelOutOfRange,
}

#[derive(Clone, Debug, Default)]
pub struct ScaleOctaveTuning {
    pub c: Ratio,
    pub csh: Ratio,
    pub d: Ratio,
    pub dsh: Ratio,
    pub e: Ratio,
    pub f: Ratio,
    pub fsh: Ratio,
    pub g: Ratio,
    pub gsh: Ratio,
    pub a: Ratio,
    pub ash: Ratio,
    pub b: Ratio,
}

impl ScaleOctaveTuning {
    pub fn as_mut(&mut self, letter: NoteLetter) -> &mut Ratio {
        match letter {
            NoteLetter::C => &mut self.c,
            NoteLetter::Csh => &mut self.csh,
            NoteLetter::D => &mut self.d,
            NoteLetter::Dsh => &mut self.dsh,
            NoteLetter::E => &mut self.e,
            NoteLetter::F => &mut self.f,
            NoteLetter::Fsh => &mut self.fsh,
            NoteLetter::G => &mut self.g,
            NoteLetter::Gsh => &mut self.gsh,
            NoteLetter::A => &mut self.a,
            NoteLetter::Ash => &mut self.ash,
            NoteLetter::B => &mut self.b,
        }
    }
}
pub enum Channels {
    All,
    Some(HashSet<u8>),
}

impl From<HashSet<u8>> for Channels {
    fn from(channels: HashSet<u8>) -> Self {
        Self::Some(channels)
    }
}

impl From<u8> for Channels {
    fn from(channel: u8) -> Self {
        Self::Some(iter::once(channel).collect())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DeviceId(u8);

impl DeviceId {
    pub fn broadcast() -> Self {
        DeviceId(DEVICE_ID_BROADCAST)
    }

    pub fn from(device_id: u8) -> Option<Self> {
        if device_id < 128 {
            Some(DeviceId(device_id))
        } else {
            None
        }
    }

    pub fn as_u8(self) -> u8 {
        self.0
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::broadcast()
    }
}

pub fn tuning_program_change(channel: u8, tuning_program: u8) -> Option<[ChannelMessage; 3]> {
    const TUNING_PROGRAM_CHANGE_MSB: u8 = 0x00;
    const TUNING_PROGRAM_CHANGE_LSB: u8 = 0x03;

    rpn_message(
        channel,
        TUNING_PROGRAM_CHANGE_MSB,
        TUNING_PROGRAM_CHANGE_LSB,
        tuning_program,
    )
}

pub fn tuning_bank_change(channel: u8, tuning_bank: u8) -> Option<[ChannelMessage; 3]> {
    const TUNING_BANK_CHANGE_MSB: u8 = 0x00;
    const TUNING_BANK_CHANGE_LSB: u8 = 0x04;

    rpn_message(
        channel,
        TUNING_BANK_CHANGE_MSB,
        TUNING_BANK_CHANGE_LSB,
        tuning_bank,
    )
}

fn rpn_message(
    channel: u8,
    parameter_number_msb: u8,
    parameter_number_lsb: u8,
    value: u8,
) -> Option<[ChannelMessage; 3]> {
    Some([
        ChannelMessageType::ControlChange {
            controller: 0x65,
            value: parameter_number_msb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: 0x64,
            value: parameter_number_lsb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: 0x06,
            value,
        }
        .in_channel(channel)?,
    ])
}

#[cfg(test)]
mod test {
    use std::iter::FromIterator;

    use crate::{
        note::NoteLetter,
        scala::{KbmRoot, Scl},
    };

    use super::*;

    #[test]
    fn octave_tuning() {
        let test_cases: &[(&[_], _, _, _)] = &[
            (&[], 0b0000_0000, 0b0000_0000, 0b0000_0000),
            (&[0], 0b0000_0000, 0b0000_0000, 0b0000_0001),
            (&[6], 0b0000_0000, 0b0000_0000, 0b0100_0000),
            (&[7], 0b0000_0000, 0b0000_0001, 0b0000_0000),
            (&[13], 0b0000_0000, 0b0100_0000, 0b0000_0000),
            (&[14], 0b0000_0001, 0b0000_0000, 0b0000_0000),
            (&[15], 0b0000_0010, 0b0000_0000, 0b0000_0000),
            (
                &[0, 2, 4, 6, 8, 10, 12, 14],
                0b0000_0001,
                0b0010_1010,
                0b0101_0101,
            ),
            (
                &[1, 3, 5, 7, 9, 11, 13, 15],
                0b0000_0010,
                0b0101_0101,
                0b0010_1010,
            ),
        ];

        let octave_tuning = ScaleOctaveTuning {
            c: Ratio::from_cents(-61.0),
            csh: Ratio::from_cents(-50.0),
            d: Ratio::from_cents(-39.0),
            dsh: Ratio::from_cents(-28.0),
            e: Ratio::from_cents(-17.0),
            f: Ratio::from_cents(-6.0),
            fsh: Ratio::from_cents(5.0),
            g: Ratio::from_cents(16.0),
            gsh: Ratio::from_cents(27.0),
            a: Ratio::from_cents(38.0),
            ash: Ratio::from_cents(49.0),
            b: Ratio::from_cents(60.0),
        };

        for (channels, expected_channel_byte_1, expected_channel_byte_2, expected_channel_byte_3) in
            test_cases.iter()
        {
            let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                &octave_tuning,
                Channels::Some(channels.iter().cloned().collect()),
                DeviceId::from(77).unwrap(),
            )
            .unwrap();

            assert_eq!(
                tuning_message.sysex_bytes(),
                [
                    0xf0,
                    0x7e,
                    77,
                    0x08,
                    0x08,
                    *expected_channel_byte_1,
                    *expected_channel_byte_2,
                    *expected_channel_byte_3,
                    0x40 - 61,
                    0x40 - 50,
                    0x40 - 39,
                    0x40 - 28,
                    0x40 - 17,
                    0x40 - 6,
                    0x40 + 5,
                    0x40 + 16,
                    0x40 + 27,
                    0x40 + 38,
                    0x40 + 49,
                    0x40 + 60,
                    0xf7
                ]
            );
        }
    }

    #[test]
    fn octave_tuning_default_values() {
        let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
            &Default::default(),
            Channels::All,
            Default::default(),
        )
        .unwrap();

        assert_eq!(
            tuning_message.sysex_bytes(),
            [
                0xf0,
                0x7e,
                0x7f,
                0x08,
                0x08,
                0b0000_0011,
                0b0111_1111,
                0b0111_1111,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                64,
                0xf7
            ]
        );
    }

    #[test]
    fn single_note_tuning() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(31))
            .build()
            .unwrap();
        let kbm = KbmRoot::from(NoteLetter::D.in_octave(4));
        let tuning = (scl, kbm);

        let single_message = SingleNoteTuningChangeMessage::from_tuning(
            &tuning,
            (0..127).map(PianoKey::from_midi_number),
            DeviceId::from(11).unwrap(),
            22,
        )
        .unwrap();
        assert_eq!(
            Vec::from_iter(single_message.sysex_bytes()),
            [[
                0xf0, 0x7f, 11, 0x08, 0x02, 22, 127, 0, 38, 0, 0, 1, 38, 49, 70, 2, 38, 99, 12, 3,
                39, 20, 83, 4, 39, 70, 25, 5, 39, 119, 95, 6, 40, 41, 37, 7, 40, 90, 107, 8, 41,
                12, 50, 9, 41, 61, 120, 10, 41, 111, 62, 11, 42, 33, 4, 12, 42, 82, 74, 13, 43, 4,
                17, 14, 43, 53, 87, 15, 43, 103, 29, 16, 44, 24, 99, 17, 44, 74, 41, 18, 44, 123,
                111, 19, 45, 45, 54, 20, 45, 94, 124, 21, 46, 16, 66, 22, 46, 66, 8, 23, 46, 115,
                78, 24, 47, 37, 21, 25, 47, 86, 91, 26, 48, 8, 33, 27, 48, 57, 103, 28, 48, 107,
                45, 29, 49, 28, 116, 30, 49, 78, 58, 31, 50, 0, 0, 32, 50, 49, 70, 33, 50, 99, 12,
                34, 51, 20, 83, 35, 51, 70, 25, 36, 51, 119, 95, 37, 52, 41, 37, 38, 52, 90, 107,
                39, 53, 12, 50, 40, 53, 61, 120, 41, 53, 111, 62, 42, 54, 33, 4, 43, 54, 82, 74,
                44, 55, 4, 17, 45, 55, 53, 87, 46, 55, 103, 29, 47, 56, 24, 99, 48, 56, 74, 41, 49,
                56, 123, 111, 50, 57, 45, 54, 51, 57, 94, 124, 52, 58, 16, 66, 53, 58, 66, 8, 54,
                58, 115, 78, 55, 59, 37, 21, 56, 59, 86, 91, 57, 60, 8, 33, 58, 60, 57, 103, 59,
                60, 107, 45, 60, 61, 28, 116, 61, 61, 78, 58, 62, 62, 0, 0, 63, 62, 49, 70, 64, 62,
                99, 12, 65, 63, 20, 83, 66, 63, 70, 25, 67, 63, 119, 95, 68, 64, 41, 37, 69, 64,
                90, 107, 70, 65, 12, 50, 71, 65, 61, 120, 72, 65, 111, 62, 73, 66, 33, 4, 74, 66,
                82, 74, 75, 67, 4, 17, 76, 67, 53, 87, 77, 67, 103, 29, 78, 68, 24, 99, 79, 68, 74,
                41, 80, 68, 123, 111, 81, 69, 45, 54, 82, 69, 94, 124, 83, 70, 16, 66, 84, 70, 66,
                8, 85, 70, 115, 78, 86, 71, 37, 21, 87, 71, 86, 91, 88, 72, 8, 33, 89, 72, 57, 103,
                90, 72, 107, 45, 91, 73, 28, 116, 92, 73, 78, 58, 93, 74, 0, 0, 94, 74, 49, 70, 95,
                74, 99, 12, 96, 75, 20, 83, 97, 75, 70, 25, 98, 75, 119, 95, 99, 76, 41, 37, 100,
                76, 90, 107, 101, 77, 12, 50, 102, 77, 61, 120, 103, 77, 111, 62, 104, 78, 33, 4,
                105, 78, 82, 74, 106, 79, 4, 17, 107, 79, 53, 87, 108, 79, 103, 29, 109, 80, 24,
                99, 110, 80, 74, 41, 111, 80, 123, 111, 112, 81, 45, 54, 113, 81, 94, 124, 114, 82,
                16, 66, 115, 82, 66, 8, 116, 82, 115, 78, 117, 83, 37, 21, 118, 83, 86, 91, 119,
                84, 8, 33, 120, 84, 57, 103, 121, 84, 107, 45, 122, 85, 28, 116, 123, 85, 78, 58,
                124, 86, 0, 0, 125, 86, 49, 70, 126, 86, 99, 12, 0xf7
            ]]
        );

        let split_message = SingleNoteTuningChangeMessage::from_tuning(
            &tuning,
            (0..128).map(PianoKey::from_midi_number),
            DeviceId::from(33).unwrap(),
            44,
        )
        .unwrap();
        assert_eq!(
            Vec::from_iter(split_message.sysex_bytes()),
            [
                [
                    0xf0, 0x7f, 33, 0x08, 0x02, 44, 64, 0, 38, 0, 0, 1, 38, 49, 70, 2, 38, 99, 12,
                    3, 39, 20, 83, 4, 39, 70, 25, 5, 39, 119, 95, 6, 40, 41, 37, 7, 40, 90, 107, 8,
                    41, 12, 50, 9, 41, 61, 120, 10, 41, 111, 62, 11, 42, 33, 4, 12, 42, 82, 74, 13,
                    43, 4, 17, 14, 43, 53, 87, 15, 43, 103, 29, 16, 44, 24, 99, 17, 44, 74, 41, 18,
                    44, 123, 111, 19, 45, 45, 54, 20, 45, 94, 124, 21, 46, 16, 66, 22, 46, 66, 8,
                    23, 46, 115, 78, 24, 47, 37, 21, 25, 47, 86, 91, 26, 48, 8, 33, 27, 48, 57,
                    103, 28, 48, 107, 45, 29, 49, 28, 116, 30, 49, 78, 58, 31, 50, 0, 0, 32, 50,
                    49, 70, 33, 50, 99, 12, 34, 51, 20, 83, 35, 51, 70, 25, 36, 51, 119, 95, 37,
                    52, 41, 37, 38, 52, 90, 107, 39, 53, 12, 50, 40, 53, 61, 120, 41, 53, 111, 62,
                    42, 54, 33, 4, 43, 54, 82, 74, 44, 55, 4, 17, 45, 55, 53, 87, 46, 55, 103, 29,
                    47, 56, 24, 99, 48, 56, 74, 41, 49, 56, 123, 111, 50, 57, 45, 54, 51, 57, 94,
                    124, 52, 58, 16, 66, 53, 58, 66, 8, 54, 58, 115, 78, 55, 59, 37, 21, 56, 59,
                    86, 91, 57, 60, 8, 33, 58, 60, 57, 103, 59, 60, 107, 45, 60, 61, 28, 116, 61,
                    61, 78, 58, 62, 62, 0, 0, 63, 62, 49, 70, 0xf7
                ],
                [
                    0xf0, 0x7f, 33, 0x08, 0x02, 44, 64, 64, 62, 99, 12, 65, 63, 20, 83, 66, 63, 70,
                    25, 67, 63, 119, 95, 68, 64, 41, 37, 69, 64, 90, 107, 70, 65, 12, 50, 71, 65,
                    61, 120, 72, 65, 111, 62, 73, 66, 33, 4, 74, 66, 82, 74, 75, 67, 4, 17, 76, 67,
                    53, 87, 77, 67, 103, 29, 78, 68, 24, 99, 79, 68, 74, 41, 80, 68, 123, 111, 81,
                    69, 45, 54, 82, 69, 94, 124, 83, 70, 16, 66, 84, 70, 66, 8, 85, 70, 115, 78,
                    86, 71, 37, 21, 87, 71, 86, 91, 88, 72, 8, 33, 89, 72, 57, 103, 90, 72, 107,
                    45, 91, 73, 28, 116, 92, 73, 78, 58, 93, 74, 0, 0, 94, 74, 49, 70, 95, 74, 99,
                    12, 96, 75, 20, 83, 97, 75, 70, 25, 98, 75, 119, 95, 99, 76, 41, 37, 100, 76,
                    90, 107, 101, 77, 12, 50, 102, 77, 61, 120, 103, 77, 111, 62, 104, 78, 33, 4,
                    105, 78, 82, 74, 106, 79, 4, 17, 107, 79, 53, 87, 108, 79, 103, 29, 109, 80,
                    24, 99, 110, 80, 74, 41, 111, 80, 123, 111, 112, 81, 45, 54, 113, 81, 94, 124,
                    114, 82, 16, 66, 115, 82, 66, 8, 116, 82, 115, 78, 117, 83, 37, 21, 118, 83,
                    86, 91, 119, 84, 8, 33, 120, 84, 57, 103, 121, 84, 107, 45, 122, 85, 28, 116,
                    123, 85, 78, 58, 124, 86, 0, 0, 125, 86, 49, 70, 126, 86, 99, 12, 127, 87, 20,
                    83, 0xf7
                ]
            ]
        );
    }

    #[test]
    fn single_note_tuning_empty_tuning_change_list() {
        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            iter::empty(),
            Default::default(),
            55,
        )
        .unwrap();

        let expected_message = [0xf0, 0x7f, 127, 0x08, 0x02, 55, 0, 0xf7];

        assert_eq!(
            Vec::from_iter(tuning_message.sysex_bytes()),
            [expected_message]
        );
        assert_eq!(tuning_message.retuned_notes().len(), 0);
        assert_eq!(tuning_message.out_of_range_notes().len(), 0);
    }

    #[test]
    fn too_many_tuning_changes() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(7))
            .build()
            .unwrap();
        let kbm = KbmRoot::from(NoteLetter::D.in_octave(4));
        let tuning = (scl, kbm);

        let result = SingleNoteTuningChangeMessage::from_tuning(
            &tuning,
            (0..129).map(PianoKey::from_midi_number),
            DeviceId::from(11).unwrap(),
            22,
        );
        assert!(matches!(
            result,
            Err(SingleNoteTuningChangeError::TuningChangeListTooLong)
        ));
    }

    #[test]
    fn single_note_tuning_program_out_of_range() {
        let result = SingleNoteTuningChangeMessage::from_tuning_changes(
            iter::empty(),
            Default::default(),
            128,
        );

        assert!(matches!(
            result,
            Err(SingleNoteTuningChangeError::TuningProgramOutOfRange)
        ));
    }

    #[test]
    fn single_note_tuning_numerical_correctness() {
        let tuning_changes = [
            (11, -1.0),     // Out of range
            (22, -0.00004), // Out of range
            (33, -0.00003), // Numerically equivalent to 0
            (44, 0.0),
            (55, 0.00003),  // Numerically equivalent to 0
            (66, 0.00004),  // Smallest value above 0 => lsb = 1
            (77, 31.41592), // Random number => (msb, lsb) = (53, 30)
            (11, 62.83185), // Random number => (msb, lsb) = (106, 61)
            (22, 68.99996), // Smallest value below 69 => lsb = 127
            (33, 68.99997), // Numerically equivalent to 69
            (44, 69.0),
            (55, 69.00003),  // Numerically equivalent to 69
            (66, 69.00004),  // Smallest value above 69 => lsb = 1
            (77, 69.25),     // 25% of a semitone => msb = 32
            (11, 69.49996),  // Smallest value below 69.5 => lsb = 127
            (22, 69.49997),  // Numerically equivalent to 69.5
            (33, 69.5),      // 50% of a semitone => msb = 64
            (44, 69.50003),  // Numerically equivalent to 69.5
            (55, 69.50004),  // Smallest value above 69.5 => lsb = 1
            (66, 69.75),     // 75% of a semitone => msb = 96
            (77, 127.99996), // Smallest value below 128 => lsb = 127
            (1, 127.99997),  // Out of range
            (11, 129.0),     // Out of range
        ]
        .iter()
        .map(|&(source, target)| {
            let key = PianoKey::from_midi_number(source);
            let pitch = Note::from_midi_number(0).pitch() * Ratio::from_semitones(target);
            SingleNoteTuningChange::new(key, pitch)
        });

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            tuning_changes,
            DeviceId::from(88).unwrap(),
            99,
        )
        .unwrap();

        assert_eq!(
            Vec::from_iter(tuning_message.sysex_bytes()),
            [[
                0xf0, 0x7f, 88, 0x08, 0x02, 99, 19, 33, 0, 0, 0, 44, 0, 0, 0, 55, 0, 0, 0, 66, 0,
                0, 1, 77, 31, 53, 30, 11, 62, 106, 61, 22, 68, 127, 127, 33, 69, 0, 0, 44, 69, 0,
                0, 55, 69, 0, 0, 66, 69, 0, 1, 77, 69, 32, 0, 11, 69, 63, 127, 22, 69, 64, 0, 33,
                69, 64, 0, 44, 69, 64, 0, 55, 69, 64, 1, 66, 69, 96, 0, 77, 127, 127, 127, 0xf7,
            ]]
        );
        assert_eq!(tuning_message.retuned_notes().len(), 19);
        assert_eq!(tuning_message.out_of_range_notes().len(), 4);
    }
}
