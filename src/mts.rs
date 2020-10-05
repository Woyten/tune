//! Communication with devices over the MIDI Tuning Standard.
//!
//! References:
//! - [Sysex messages](https://www.midi.org/specifications/item/table-4-universal-system-exclusive-messages)
//! - [MIDI Tuning Standard](http://www.microtonal-synthesis.com/MIDItuning.html)

use crate::ratio::Ratio;
use crate::{key::PianoKey, note::NoteLetter, tuning::Tuning};
use core::ops::Range;
use std::collections::HashSet;
use std::fmt;
use std::{fmt::Debug, iter};

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

const NOTE_RANGE: Range<u8> = 1..128; // Only 127 notes can be retuned in realtime single note tuning
const MAX_VALUE_14_BITS: f64 = (1 << 14) as f64;
const BIT_MASK_14_BITS: i32 = (1 << 14) - 1;
const BIT_MASK_7_BITS: i32 = (1 << 7) - 1;

#[derive(Clone)]
pub struct SingleNoteTuningChangeMessage {
    sysex_call: Vec<u8>,
    retuned_notes: Vec<SingleNoteTuningChange>,
    out_of_range_notes: Vec<SingleNoteTuningChange>,
}

impl SingleNoteTuningChangeMessage {
    pub fn from_scale(
        tuning: impl Tuning<PianoKey>,
        device_id: DeviceId,
        tuning_program: u8,
    ) -> Result<Self, TuningError> {
        let tuning_changes = NOTE_RANGE.map(move |note_number| {
            let approximation = tuning
                .pitch_of(PianoKey::from_midi_number(i32::from(note_number)))
                .find_in(&());
            let target_midi_number = approximation.approx_value.midi_number();
            SingleNoteTuningChange::new(note_number, target_midi_number, approximation.deviation)
        });
        Self::from_tuning_changes(tuning_changes, device_id, tuning_program)
    }

    pub fn from_tuning_changes(
        tuning_changes: impl IntoIterator<Item = SingleNoteTuningChange>,
        device_id: DeviceId,
        tuning_program: u8,
    ) -> Result<Self, TuningError> {
        if tuning_program >= 128 {
            return Err(TuningError::TuningProgramNumberOutOfRange(tuning_program));
        }

        let mut result = SingleNoteTuningChangeMessage {
            sysex_call: Vec::new(),
            retuned_notes: Vec::new(),
            out_of_range_notes: Vec::new(),
        };

        result.sysex_call.push(SYSEX_START);
        result.sysex_call.push(SYSEX_RT);
        result.sysex_call.push(device_id.as_u8());
        result.sysex_call.push(MIDI_TUNING_STANDARD);
        result.sysex_call.push(SINGLE_NOTE_TUNING_CHANGE);
        result.sysex_call.push(tuning_program);
        let number_of_notes_index = result.sysex_call.len();
        result.sysex_call.push(0); // Number of notes
        for single_note_tuning in tuning_changes {
            result.push_tuning_change(single_note_tuning.normalized())?;
        }
        result.sysex_call[number_of_notes_index] = result.retuned_notes.len() as u8;
        result.sysex_call.push(SYSEX_END);

        Ok(result)
    }

    fn push_tuning_change(
        &mut self,
        tuning_change: SingleNoteTuningChange,
    ) -> Result<(), TuningError> {
        if (0..128).contains(&tuning_change.target_note) {
            let pitch_msb = tuning_change.detune_as_14_bits >> 7;
            let pitch_lsb = tuning_change.detune_as_14_bits & BIT_MASK_7_BITS;

            self.sysex_call
                .push(check_source_note(tuning_change.source_note)?);
            self.sysex_call.push(tuning_change.target_note as u8);
            self.sysex_call.push(pitch_msb as u8);
            self.sysex_call.push(pitch_lsb as u8);

            self.retuned_notes.push(tuning_change);
        } else {
            self.out_of_range_notes.push(tuning_change);
        }
        Ok(())
    }

    pub fn sysex_bytes(&self) -> &[u8] {
        &self.sysex_call
    }

    pub fn retuned_notes(&self) -> &[SingleNoteTuningChange] {
        &self.retuned_notes
    }

    pub fn out_of_range_notes(&self) -> &[SingleNoteTuningChange] {
        &self.out_of_range_notes
    }
}

impl Debug for SingleNoteTuningChangeMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.sysex_call {
            writeln!(f, "0x{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SingleNoteTuningChange {
    source_note: u8,
    target_note: i32,
    detune_as_14_bits: i32,
}

impl SingleNoteTuningChange {
    pub fn new(source_note: u8, target_note: i32, detune: Ratio) -> Self {
        SingleNoteTuningChange {
            source_note,
            target_note,
            detune_as_14_bits: (detune.as_semitones() * MAX_VALUE_14_BITS).round() as i32,
        }
    }

    fn normalized(self) -> Self {
        SingleNoteTuningChange {
            target_note: self.target_note + (self.detune_as_14_bits >> 14),
            detune_as_14_bits: self.detune_as_14_bits & BIT_MASK_14_BITS,
            ..self
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
    ) -> Result<Self, TuningError> {
        let channels = channels.into();

        let mut sysex_call = Vec::new();
        sysex_call.push(SYSEX_START);
        sysex_call.push(SYSEX_NON_RT);
        sysex_call.push(device_id.as_u8());
        sysex_call.push(MIDI_TUNING_STANDARD);
        sysex_call.push(SCALE_OCTAVE_TUNING_1_BYTE_FORMAT);
        match channels {
            Channels::All => {
                sysex_call.push(0b0000_0011); // bits 0 to 1 = channel 15 to 16
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 8 to 14
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 1 to 7
            }
            Channels::Some(channels) => {
                sysex_call.push(encode_channels(&channels, 14..16));
                sysex_call.push(encode_channels(&channels, 7..14));
                sysex_call.push(encode_channels(&channels, 0..7));
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
            sysex_call.push(convert_pitch_bend(pitch_bend));
        }
        sysex_call.push(SYSEX_END);

        Ok(ScaleOctaveTuningMessage { sysex_call })
    }

    pub fn sysex_bytes(&self) -> &[u8] {
        &self.sysex_call
    }
}

fn convert_pitch_bend(pitch_bend: Ratio) -> u8 {
    let cents_value = pitch_bend.as_cents().round();
    assert!((-64.0..63.0).contains(&cents_value));
    (0x40 + cents_value as i8) as u8
}

fn encode_channels(
    selected_channels: &HashSet<u8>,
    channels_to_encode: impl IntoIterator<Item = u8>,
) -> u8 {
    let mut channel_byte = 0;
    for (position, channel_to_encode) in channels_to_encode.into_iter().enumerate() {
        channel_byte |= (selected_channels.contains(&channel_to_encode) as u8) << position;
    }
    channel_byte
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

fn check_source_note(source_note: u8) -> Result<u8, TuningError> {
    if (1..128).contains(&source_note) {
        Ok(source_note)
    } else {
        Err(TuningError::SourceNoteOutOfRange(source_note))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum TuningError {
    SourceNoteOutOfRange(u8),
    TuningChangeListTooLong(usize),
    TuningProgramNumberOutOfRange(u8),
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::ratio::Ratio;

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
            (&[16], 0b0000_0000, 0b0000_0000, 0b0000_0000),
            (
                &[0, 2, 4, 6, 8, 10, 12, 14, 98],
                0b0000_0001,
                0b0010_1010,
                0b0101_0101,
            ),
            (
                &[1, 3, 5, 7, 9, 11, 13, 15, 99],
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
        let test_cases = [
            (-10.0, 65, 0, 0),
            (-2.0, 73, 0, 0),
            (-1.5, 73, 64, 0),
            (-1.0, 74, 0, 0),
            (-0.9, 74, 12, 102),
            (-0.5, 74, 64, 0),
            (-0.1, 74, 115, 26),
            (0.0, 75, 0, 0),
            (0.1, 75, 12, 102),
            (0.5, 75, 64, 0),
            (0.9, 75, 115, 26),
            (1.0, 76, 0, 0),
            (1.5, 76, 64, 0),
            (2.0, 77, 0, 0),
            (10.0, 85, 0, 0),
        ];

        for &(detune, expected_target_note, expected_pitch_msb, expected_pitch_lsb) in
            test_cases.iter()
        {
            let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
                vec![SingleNoteTuningChange::new(
                    70,
                    75,
                    Ratio::from_semitones(detune),
                )],
                DeviceId::from(33).unwrap(),
                99,
            )
            .unwrap();

            assert_eq!(
                tuning_message.sysex_bytes(),
                [
                    0xf0,
                    0x7f,
                    33,
                    0x08,
                    0x02,
                    99,
                    1,
                    70,
                    expected_target_note,
                    expected_pitch_msb,
                    expected_pitch_lsb,
                    0xf7
                ]
            );
            assert_eq!(tuning_message.retuned_notes().len(), 1);
            assert_eq!(tuning_message.out_of_range_notes().len(), 0);
        }
    }

    #[test]
    fn single_note_tuning_multiple_notes() {
        let notes_to_tune = vec![(70, 75, -0.1), (80, 85, 0.1), (90, 95, 0.5)]
            .into_iter()
            .map(|(source, target, detune)| {
                SingleNoteTuningChange::new(source, target, Ratio::from_semitones(detune))
            });

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            notes_to_tune,
            Default::default(),
            66,
        )
        .unwrap();

        assert_eq!(
            tuning_message.sysex_bytes(),
            [
                0xf0, 0x7f, 0x7f, 0x08, 0x02, 66, 3, 70, 74, 115, 26, 80, 85, 12, 102, 90, 95, 64,
                0, 0xf7
            ]
        );
        assert_eq!(tuning_message.retuned_notes().len(), 3);
        assert_eq!(tuning_message.out_of_range_notes().len(), 0);
    }

    #[test]
    fn failures() {
        let notes_to_tune = vec![
            (11, 0, -0.5), // failure
            (22, 0, 0.0),
            (33, 0, 0.5),
            (44, 128, -0.5),
            (55, 128, 0.0), // failure
            (66, 120, 8.0), // failure
        ]
        .into_iter()
        .map(|(source, target, detune)| {
            SingleNoteTuningChange::new(source, target, Ratio::from_semitones(detune))
        });

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            notes_to_tune,
            Default::default(),
            0,
        )
        .unwrap();

        assert_eq!(
            tuning_message.sysex_bytes(),
            [0xf0, 0x7f, 0x7f, 0x08, 0x02, 0, 3, 22, 0, 0, 0, 33, 0, 64, 0, 44, 127, 64, 0, 0xf7]
        );
        assert_eq!(tuning_message.retuned_notes().len(), 3);
        assert_eq!(tuning_message.out_of_range_notes().len(), 3);
    }
}
