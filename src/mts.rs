//! Communication with devices over the MIDI Tuning Standard.
//!
//! References:
//! - [Sysex messages](https://www.midi.org/specifications-old/item/table-4-universal-system-exclusive-messages)
//! - [MIDI Tuning Standard](https://musescore.org/sites/musescore.org/files/2018-06/midituning.pdf)

use std::{collections::HashSet, fmt::Debug, iter};

use crate::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    note::NoteLetter,
    pitch::{Pitch, Pitched, Ratio},
    tuning::KeyboardMapping,
};

// Universal System Exclusive Messages
// f0 7e <payload> f7 Non-Real Time
// f0 7f <payload> f7 Real Time

const SYSEX_START: u8 = 0xf0;
const SYSEX_NON_RT: u8 = 0x7e;
const SYSEX_RT: u8 = 0x7f;
const SYSEX_END: u8 = 0xf7;

// MIDI Tuning Standard
// 08 02 Single Note Tuning Change
// 08 07 Single Note Tuning Change with Bank Select
// 08 08 Scale/Octave Tuning, 1 byte format
// 08 09 Scale/Octave Tuning, 2 byte format

const MIDI_TUNING_STANDARD: u8 = 0x08;

const SINGLE_NOTE_TUNING_CHANGE: u8 = 0x02;
const SINGLE_NOTE_TUNING_CHANGE_WITH_BANK_SELECT: u8 = 0x07;
const SCALE_OCTAVE_TUNING_1_BYTE_FORMAT: u8 = 0x08;
const SCALE_OCTAVE_TUNING_2_BYTE_FORMAT: u8 = 0x09;

const DEVICE_ID_BROADCAST: u8 = 0x7f;

const U7_MASK: u16 = (1 << 7) - 1;
const U14_UPPER_BOUND_AS_F64: f64 = (1 << 14) as f64;

/// Properties of the generated *Single Note Tuning Change* message.
///
/// # Examples
///
/// ```
/// # use tune::mts::SingleNoteTuningChange;
/// # use tune::mts::SingleNoteTuningChangeMessage;
/// # use tune::mts::SingleNoteTuningChangeOptions;
/// # use tune::note::NoteLetter;
/// # use tune::pitch::Pitch;
/// let a4 = NoteLetter::A.in_octave(4).as_piano_key();
/// let target_pitch = Pitch::from_hz(445.0);
///
/// let tuning_change = SingleNoteTuningChange { key: a4, target_pitch };
///
/// // Use default options
/// let options = SingleNoteTuningChangeOptions::default();
///
/// let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
///     &options,
///     std::iter::once(tuning_change),
/// )
/// .unwrap();
///
/// assert_eq!(
///     Vec::from_iter(tuning_message.sysex_bytes()),
///     [[0xf0, 0x7f, 0x7f, 0x08, 0x02, // RT Single Note Tuning Change
///       0, 1,                         // Tuning program / number of changes
///       69, 69, 25, 5,                // Tuning changes
///       0xf7]]                        // Sysex end
/// );
///
/// // Use custom options
/// let options = SingleNoteTuningChangeOptions {
///     realtime: false,
///     device_id: 55,
///     tuning_program: 66,
///     with_bank_select: Some(77),
/// };
///
/// let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
///     &options,
///     std::iter::once(tuning_change),
/// )
/// .unwrap();
///
/// assert_eq!(
///     Vec::from_iter(tuning_message.sysex_bytes()),
///     [[0xf0, 0x7e, 55, 0x08, 0x07, // Non-RT Single Note Tuning Change with Bank Select
///       77, 66, 1,                  // Tuning program / tuning bank / number of changes
///       69, 69, 25, 5,              // Tuning changes
///       0xf7]]                      // Sysex end
/// );
/// ```
#[derive(Copy, Clone, Debug)]
pub struct SingleNoteTuningChangeOptions {
    /// If set to true, generate a realtime SysEx message (defaults to `true`).
    pub realtime: bool,

    /// Specifies the device ID (defaults to broadcast/0x7f).
    pub device_id: u8,

    /// Specifies the tuning program to be affected (defaults to 0).
    pub tuning_program: u8,

    /// If given, generate a *Single Note Tuning Change with Bank Select* message.
    pub with_bank_select: Option<u8>,
}

impl Default for SingleNoteTuningChangeOptions {
    fn default() -> Self {
        Self {
            realtime: true,
            device_id: DEVICE_ID_BROADCAST,
            tuning_program: 0,
            with_bank_select: None,
        }
    }
}

/// Retunes one or multiple MIDI notes using the *Single Note Tuning Change* message format.
#[derive(Clone, Debug)]
pub struct SingleNoteTuningChangeMessage {
    sysex_calls: [Option<Vec<u8>>; 2],
    out_of_range_notes: Vec<SingleNoteTuningChange>,
}

impl SingleNoteTuningChangeMessage {
    /// Creates a [`SingleNoteTuningChangeMessage`] from the provided `tuning` and `keys`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::key::PianoKey;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::KbmRoot;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_ratio(Ratio::octave().divided_into_equal_steps(7))
    ///     .build()
    ///     .unwrap();
    /// let kbm = KbmRoot::from(NoteLetter::D.in_octave(4)).to_kbm();
    ///
    /// let tuning_message = SingleNoteTuningChangeMessage::from_tuning(
    ///     &Default::default(),
    ///     (scl, kbm),
    ///     (21..109).map(PianoKey::from_midi_number),
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(tuning_message.sysex_bytes().count(), 1);
    /// assert_eq!(tuning_message.out_of_range_notes().len(), 13);
    /// ```
    pub fn from_tuning(
        options: &SingleNoteTuningChangeOptions,
        tuning: impl KeyboardMapping<PianoKey>,
        keys: impl IntoIterator<Item = PianoKey>,
    ) -> Result<Self, SingleNoteTuningChangeError> {
        let tuning_changes = keys.into_iter().flat_map(|key| {
            tuning
                .maybe_pitch_of(key)
                .map(|target_pitch| SingleNoteTuningChange { key, target_pitch })
        });
        Self::from_tuning_changes(options, tuning_changes)
    }

    /// Creates a [`SingleNoteTuningChangeMessage`] from the provided `tuning_changes`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::mts::SingleNoteTuningChange;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::note::NoteLetter;
    /// # use tune::pitch::Pitch;
    /// let key = NoteLetter::A.in_octave(4).as_piano_key();
    ///
    /// let good = SingleNoteTuningChange { key, target_pitch: Pitch::from_hz(445.0) };
    /// let too_low = SingleNoteTuningChange { key, target_pitch: Pitch::from_hz(1.0) };
    /// let too_high = SingleNoteTuningChange { key, target_pitch: Pitch::from_hz(100000.0) };
    ///
    /// let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
    ///     &Default::default(), [good, too_low, too_high]
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(tuning_message.sysex_bytes().count(), 1);
    /// assert_eq!(tuning_message.out_of_range_notes(), [too_low, too_high]);
    /// ```
    pub fn from_tuning_changes(
        options: &SingleNoteTuningChangeOptions,
        tuning_changes: impl IntoIterator<Item = SingleNoteTuningChange>,
    ) -> Result<Self, SingleNoteTuningChangeError> {
        if options.device_id >= 128 {
            return Err(SingleNoteTuningChangeError::DeviceIdOutOfRange);
        }
        if options.tuning_program >= 128 {
            return Err(SingleNoteTuningChangeError::TuningProgramOutOfRange);
        }
        if options
            .with_bank_select
            .filter(|&tuning_bank| tuning_bank >= 128)
            .is_some()
        {
            return Err(SingleNoteTuningChangeError::TuningBankNumberOutOfRange);
        }

        let mut sysex_tuning_list = Vec::new();
        let mut num_retuned_notes = 0;
        let mut out_of_range_notes = Vec::new();

        for tuning_change in tuning_changes {
            let approximation = tuning_change.target_pitch.find_in_tuning(());
            let mut target_note = approximation.approx_value;

            let mut detune_in_u14_resolution =
                (approximation.deviation.as_semitones() * U14_UPPER_BOUND_AS_F64).round();

            // Make sure that the detune range is [0c..100c] instead of [-50c..50c]
            if detune_in_u14_resolution < 0.0 {
                target_note = target_note.plus_semitones(-1);
                detune_in_u14_resolution += U14_UPPER_BOUND_AS_F64;
            }

            if let (Some(source), Some(target)) = (
                tuning_change.key.checked_midi_number(),
                target_note.checked_midi_number(),
            ) {
                let pitch_msb = (detune_in_u14_resolution as u16 >> 7) as u8;
                let pitch_lsb = (detune_in_u14_resolution as u16 & U7_MASK) as u8;

                sysex_tuning_list.push(source);
                sysex_tuning_list.push(target);
                sysex_tuning_list.push(pitch_msb);
                sysex_tuning_list.push(pitch_lsb);

                num_retuned_notes += 1;
            } else {
                out_of_range_notes.push(tuning_change);
            }

            if num_retuned_notes > 128 {
                return Err(SingleNoteTuningChangeError::TuningChangeListTooLong);
            }
        }

        let create_sysex = |sysex_tuning_list: &[u8]| {
            let mut sysex_call = Vec::with_capacity(sysex_tuning_list.len() + 9);

            sysex_call.push(SYSEX_START);
            sysex_call.push(if options.realtime {
                SYSEX_RT
            } else {
                SYSEX_NON_RT
            });
            sysex_call.push(options.device_id);
            sysex_call.push(MIDI_TUNING_STANDARD);
            sysex_call.push(if options.with_bank_select.is_some() {
                SINGLE_NOTE_TUNING_CHANGE_WITH_BANK_SELECT
            } else {
                SINGLE_NOTE_TUNING_CHANGE
            });
            if let Some(with_bank_select) = options.with_bank_select {
                sysex_call.push(with_bank_select);
            }
            sysex_call.push(options.tuning_program);
            sysex_call.push((sysex_tuning_list.len() / 4).try_into().unwrap());
            sysex_call.extend(sysex_tuning_list);
            sysex_call.push(SYSEX_END);

            sysex_call
        };

        let sysex_calls = if num_retuned_notes == 0 {
            [None, None]
        } else if num_retuned_notes < 128 {
            [Some(create_sysex(&sysex_tuning_list[..])), None]
        } else {
            [
                Some(create_sysex(&sysex_tuning_list[..256])),
                Some(create_sysex(&sysex_tuning_list[256..])),
            ]
        };

        Ok(SingleNoteTuningChangeMessage {
            sysex_calls,
            out_of_range_notes,
        })
    }

    /// Returns the tuning message conforming to the MIDI tuning standard.
    ///
    /// If less than 128 notes are retuned the iterator yields a single tuning message.
    /// If the number of retuned notes is 128 two messages with a batch of 64 notes are yielded.
    /// If the number of retuned notes is 0 no message is yielded.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::mts::SingleNoteTuningChange;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::pitch::Pitched;
    /// let create_tuning_message_with_num_changes = |num_changes| {
    ///     let tuning_changes = (0..num_changes).map(|midi_number| {
    ///         SingleNoteTuningChange {
    ///             key: PianoKey::from_midi_number(midi_number),
    ///             target_pitch: Note::from_midi_number(midi_number).pitch(),
    ///         }
    ///     });
    ///
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(
    ///         &Default::default(),
    ///         tuning_changes,
    ///     )
    ///     .unwrap()
    /// };
    ///
    /// assert_eq!(create_tuning_message_with_num_changes(0).sysex_bytes().count(), 0);
    /// assert_eq!(create_tuning_message_with_num_changes(127).sysex_bytes().count(), 1);
    /// assert_eq!(create_tuning_message_with_num_changes(128).sysex_bytes().count(), 2);
    /// ```
    pub fn sysex_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.sysex_calls.iter().flatten().map(Vec::as_slice)
    }

    /// Return notes whose target pitch is not representable by the tuning message.
    pub fn out_of_range_notes(&self) -> &[SingleNoteTuningChange] {
        &self.out_of_range_notes
    }
}

/// Tunes the given [`PianoKey`] to the given [`Pitch`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SingleNoteTuningChange {
    /// The key to tune.
    pub key: PianoKey,

    /// The [`Pitch`] that the given key should sound in.
    pub target_pitch: Pitch,
}

/// Creating a [`SingleNoteTuningChangeMessage`] failed.
#[derive(Copy, Clone, Debug)]
pub enum SingleNoteTuningChangeError {
    /// The tuning change list has more than 128 elements.
    ///
    /// Discarded values are not counted.
    ///
    /// # Example
    ///
    /// ```
    /// # use tune::mts::SingleNoteTuningChange;
    /// # use tune::mts::SingleNoteTuningChangeError;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::pitch::Pitched;
    /// let vec_with_128_changes: Vec<_> = (0..128)
    ///     .map(|midi_number| {
    ///         SingleNoteTuningChange {
    ///             key: PianoKey::from_midi_number(midi_number),
    ///             target_pitch: Note::from_midi_number(midi_number).pitch(),
    ///         }
    ///     })
    ///     .collect();
    ///
    /// let mut vec_with_129_changes = vec_with_128_changes.clone();
    /// vec_with_129_changes.push({
    ///     SingleNoteTuningChange {
    ///         key: PianoKey::from_midi_number(64),
    ///         target_pitch: Note::from_midi_number(64).pitch(),
    ///     }
    /// });
    ///
    /// let mut vec_with_discarded_elements = vec_with_128_changes.clone();
    /// vec_with_discarded_elements.push({
    ///     SingleNoteTuningChange {
    ///         key: PianoKey::from_midi_number(128),
    ///         target_pitch: Note::from_midi_number(128).pitch(),
    ///     }
    /// });
    ///
    /// assert!(matches!(
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(
    ///         &Default::default(),
    ///         vec_with_128_changes,
    ///     ),
    ///     Ok(_)
    /// ));
    /// assert!(matches!(
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(
    ///         &Default::default(),
    ///         vec_with_129_changes,
    ///     ),
    ///     Err(SingleNoteTuningChangeError::TuningChangeListTooLong)
    /// ));
    /// assert!(matches!(
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(
    ///         &Default::default(),
    ///         vec_with_discarded_elements,
    ///     ),
    ///     Ok(_)
    /// ));
    /// ```
    TuningChangeListTooLong,

    /// The device ID is greater than 127.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::iter;
    /// # use tune::mts::SingleNoteTuningChangeError;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::mts::SingleNoteTuningChangeOptions;
    /// let create_tuning_message_for_device_id = |device_id| {
    ///     let options = SingleNoteTuningChangeOptions {
    ///         device_id,
    ///         ..Default::default()
    ///     };
    ///
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(&options, iter::empty())
    /// };
    ///
    /// assert!(matches!(
    ///     create_tuning_message_for_device_id(127),
    ///     Ok(_)
    /// ));
    /// assert!(matches!(
    ///     create_tuning_message_for_device_id(128),
    ///     Err(SingleNoteTuningChangeError::DeviceIdOutOfRange)
    /// ));
    /// ```
    DeviceIdOutOfRange,

    /// The tuning program number is greater than 127.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::iter;
    /// # use tune::mts::SingleNoteTuningChangeError;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::mts::SingleNoteTuningChangeOptions;
    /// let create_tuning_message_for_program = |tuning_program| {
    ///     let options = SingleNoteTuningChangeOptions {
    ///         tuning_program,
    ///         ..Default::default()
    ///     };
    ///
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(&options, iter::empty())
    /// };
    ///
    /// assert!(matches!(
    ///     create_tuning_message_for_program(127),
    ///     Ok(_)
    /// ));
    /// assert!(matches!(
    ///     create_tuning_message_for_program(128),
    ///     Err(SingleNoteTuningChangeError::TuningProgramOutOfRange)
    /// ));

    /// ```
    TuningProgramOutOfRange,

    /// The tuning bank number is greater than 127.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::iter;
    /// # use tune::mts::SingleNoteTuningChangeError;
    /// # use tune::mts::SingleNoteTuningChangeMessage;
    /// # use tune::mts::SingleNoteTuningChangeOptions;
    /// let create_tuning_message_with_bank_select = |tuning_bank| {
    ///     let options = SingleNoteTuningChangeOptions {
    ///         with_bank_select: Some(tuning_bank),
    ///         ..Default::default()
    ///     };
    ///
    ///     SingleNoteTuningChangeMessage::from_tuning_changes(&options, iter::empty())
    /// };
    ///
    /// assert!(matches!(
    ///     create_tuning_message_with_bank_select(127),
    ///     Ok(_)
    /// ));
    /// assert!(matches!(
    ///     create_tuning_message_with_bank_select(128),
    ///     Err(SingleNoteTuningChangeError::TuningBankNumberOutOfRange)
    /// ));

    /// ```
    TuningBankNumberOutOfRange,
}

/// Properties of the generated *Scale/Octave Tuning* message.
///
/// # Examples
///
/// ```
/// # use std::collections::HashSet;
/// # use tune::mts::ScaleOctaveTuning;
/// # use tune::mts::ScaleOctaveTuningFormat;
/// # use tune::mts::ScaleOctaveTuningMessage;
/// # use tune::mts::ScaleOctaveTuningOptions;
/// # use tune::note::NoteLetter;
/// # use tune::pitch::Ratio;
/// let octave_tuning = ScaleOctaveTuning {
///     c: Ratio::from_cents(10.0),
///     csh: Ratio::from_cents(-200.0), // Will be clamped
///     d: Ratio::from_cents(200.0),    // Will be clamped
///     ..Default::default()
/// };
///
/// // Use default options
/// let options = ScaleOctaveTuningOptions::default();
///
/// let tuning_message = ScaleOctaveTuningMessage::from_octave_tuning(
///     &options,
///     &octave_tuning,
/// )
/// .unwrap();
///
/// assert_eq!(
///     tuning_message.sysex_bytes(),
///     [0xf0, 0x7e, 0x7f, 0x08, 0x08,                   // Non-RT Scale/Octave Tuning (1-Byte)
///      0b00000011, 0b01111111, 0b01111111,             // Channel bits
///      74, 0, 127, 64, 64, 64, 64, 64, 64, 64, 64, 64, // Tuning changes (C - B)
///      0xf7]                                           // Sysex end
/// );
///
/// // Use custom options
/// let options = ScaleOctaveTuningOptions {
///     realtime: true,
///     device_id: 55,
///     channels: HashSet::from([0, 3, 6, 9, 12, 15]).into(),
///     format: ScaleOctaveTuningFormat::TwoByte,
/// };
///
/// let tuning_message = ScaleOctaveTuningMessage::from_octave_tuning(
///     &options,
///     &octave_tuning,
/// )
/// .unwrap();
///
/// assert_eq!(
///     tuning_message.sysex_bytes(),
///     [0xf0, 0x7f, 55, 0x08, 0x09,                  // RT Scale/Octave Tuning (2-Byte)
///      0b00000010, 0b00100100, 0b01001001,          // Channel bits
///      70, 51, 0, 0, 127, 127, 64, 0, 64, 0, 64, 0, // Tuning changes (C - F)
///      64, 0, 64, 0, 64, 0, 64, 0, 64, 0, 64, 0,    // Tuning changes (F# - B)
///      0xf7]                                        // Sysex end
/// );
/// ```
#[derive(Clone, Debug)]
pub struct ScaleOctaveTuningOptions {
    /// If set to true, generate a realtime SysEx message (defaults to `false`).
    pub realtime: bool,

    /// Specifies the device ID (defaults to broadcast/0x7f).
    pub device_id: u8,

    /// Specifies the channels that are affected by the tuning change (defaults to [`Channels::All`]).
    pub channels: Channels,

    /// Specifies whether to send a 1-byte or 2-byte message (defaults to [`ScaleOctaveTuningFormat::OneByte`]).
    pub format: ScaleOctaveTuningFormat,
}

/// 1-byte or 2-byte form of the *Scale/Octave Tuning* message.
///
/// The 1-byte form supports values in the range [-64cents..63cents], the 2-byte form supports values in the range [-100cents..100cents).
#[derive(Copy, Clone, Debug)]
pub enum ScaleOctaveTuningFormat {
    OneByte,
    TwoByte,
}

impl Default for ScaleOctaveTuningOptions {
    fn default() -> Self {
        Self {
            realtime: false,
            channels: Channels::All,
            device_id: DEVICE_ID_BROADCAST,
            format: ScaleOctaveTuningFormat::OneByte,
        }
    }
}

/// Retunes MIDI pitch classes within an octave using the *Scale/Octave Tuning* message format.
#[derive(Clone, Debug)]
pub struct ScaleOctaveTuningMessage {
    sysex_call: Vec<u8>,
}

impl ScaleOctaveTuningMessage {
    /// Creates a [`ScaleOctaveTuningMessage`] from the provided `octave_tunings`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::mts::ScaleOctaveTuning;
    /// # use tune::mts::ScaleOctaveTuningMessage;
    /// # use tune::pitch::Ratio;
    /// let octave_tuning = ScaleOctaveTuning {
    ///     c: Ratio::from_cents(10.0),
    ///     ..Default::default()
    /// };
    ///
    /// let tuning_message = ScaleOctaveTuningMessage::from_octave_tuning(
    ///     &Default::default(),
    ///     &octave_tuning,
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(tuning_message.sysex_bytes().len(), 21);
    /// ```
    pub fn from_octave_tuning(
        options: &ScaleOctaveTuningOptions,
        octave_tuning: &ScaleOctaveTuning,
    ) -> Result<Self, ScaleOctaveTuningError> {
        let mut sysex_call = Vec::with_capacity(21);

        sysex_call.push(SYSEX_START);
        sysex_call.push(if options.realtime {
            SYSEX_RT
        } else {
            SYSEX_NON_RT
        });
        sysex_call.push(options.device_id);
        sysex_call.push(MIDI_TUNING_STANDARD);
        sysex_call.push(match options.format {
            ScaleOctaveTuningFormat::OneByte => SCALE_OCTAVE_TUNING_1_BYTE_FORMAT,
            ScaleOctaveTuningFormat::TwoByte => SCALE_OCTAVE_TUNING_2_BYTE_FORMAT,
        });

        match &options.channels {
            Channels::All => {
                sysex_call.push(0b0000_0011); // bits 0 to 1 = channel 15 to 16
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 8 to 14
                sysex_call.push(0b0111_1111); // bits 0 to 6 = channel 1 to 7
            }
            Channels::Some(channels) => {
                let mut encoded_channels = [0; 3];

                for &channel in channels {
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

        let pitch_bends = [
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
        ];

        match options.format {
            ScaleOctaveTuningFormat::OneByte => {
                for pitch_bend in pitch_bends {
                    let value_to_write = (pitch_bend.as_cents() + 64.0).round().clamp(0.0, 127.0);
                    sysex_call.push(value_to_write as u8);
                }
            }
            ScaleOctaveTuningFormat::TwoByte => {
                for pitch_bend in pitch_bends {
                    let value_to_write = ((pitch_bend.as_semitones() + 1.0) * 8192.0)
                        .round()
                        .clamp(0.0, 16383.0) as u16;
                    sysex_call.push((value_to_write / 128) as u8);
                    sysex_call.push((value_to_write % 128) as u8);
                }
            }
        }

        sysex_call.push(SYSEX_END);

        Ok(ScaleOctaveTuningMessage { sysex_call })
    }

    /// Returns the tuning message conforming to the MIDI tuning standard.
    pub fn sysex_bytes(&self) -> &[u8] {
        &self.sysex_call
    }
}

/// Creating a [`ScaleOctaveTuningMessage`] failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScaleOctaveTuningError {
    /// A channel number exceeds the allowed range [0..16).
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::collections::HashSet;
    /// # use tune::mts::ScaleOctaveTuningError;
    /// # use tune::mts::ScaleOctaveTuningMessage;
    /// # use tune::mts::ScaleOctaveTuningOptions;
    /// // Channels 14 and 15 are valid
    /// let options = ScaleOctaveTuningOptions {
    ///     channels: HashSet::from([14, 15]).into(),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(matches!(
    ///     ScaleOctaveTuningMessage::from_octave_tuning(&options, &Default::default()),
    ///     Ok(_)
    /// ));
    ///
    /// // Channel 16 is invalid
    /// let options = ScaleOctaveTuningOptions {
    ///     channels: HashSet::from([14, 15, 16]).into(),
    ///     ..Default::default()
    /// };
    ///
    /// assert!(matches!(
    ///     ScaleOctaveTuningMessage::from_octave_tuning(&options, &Default::default()),
    ///     Err(ScaleOctaveTuningError::ChannelOutOfRange)
    /// ));
    /// ```
    ChannelOutOfRange,
}

/// The detuning per pitch class within an octave.
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

/// Channels to be affected by the *Scale/Octave Tuning* message.
#[derive(Clone, Debug)]
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

pub fn channel_fine_tuning(channel: u8, detuning: Ratio) -> Option<[ChannelMessage; 4]> {
    const CHANNEL_FINE_TUNING_MSB: u8 = 0x00;
    const CHANNEL_FINE_TUNING_LSB: u8 = 0x01;

    let (value_msb, value_lsb) = ratio_to_u8s(detuning);

    rpn_message_2_byte(
        channel,
        CHANNEL_FINE_TUNING_MSB,
        CHANNEL_FINE_TUNING_LSB,
        value_msb,
        value_lsb,
    )
}

pub fn tuning_program_change(channel: u8, tuning_program: u8) -> Option<[ChannelMessage; 3]> {
    const TUNING_PROGRAM_CHANGE_MSB: u8 = 0x00;
    const TUNING_PROGRAM_CHANGE_LSB: u8 = 0x03;

    rpn_message_1_byte(
        channel,
        TUNING_PROGRAM_CHANGE_MSB,
        TUNING_PROGRAM_CHANGE_LSB,
        tuning_program,
    )
}

pub fn tuning_bank_change(channel: u8, tuning_bank: u8) -> Option<[ChannelMessage; 3]> {
    const TUNING_BANK_CHANGE_MSB: u8 = 0x00;
    const TUNING_BANK_CHANGE_LSB: u8 = 0x04;

    rpn_message_1_byte(
        channel,
        TUNING_BANK_CHANGE_MSB,
        TUNING_BANK_CHANGE_LSB,
        tuning_bank,
    )
}

// RPN format reference: https://www.midi.org/specifications-old/item/table-3-control-change-messages-data-bytes-2

const RPN_MSB: u8 = 0x65;
const RPN_LSB: u8 = 0x64;
const DATA_ENTRY_MSB: u8 = 0x06;
const DATA_ENTRY_LSB: u8 = 0x26;

fn rpn_message_1_byte(
    channel: u8,
    parameter_number_msb: u8,
    parameter_number_lsb: u8,
    value: u8,
) -> Option<[ChannelMessage; 3]> {
    Some([
        ChannelMessageType::ControlChange {
            controller: RPN_MSB,
            value: parameter_number_msb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: RPN_LSB,
            value: parameter_number_lsb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: DATA_ENTRY_MSB,
            value,
        }
        .in_channel(channel)?,
    ])
}

fn rpn_message_2_byte(
    channel: u8,
    parameter_number_msb: u8,
    parameter_number_lsb: u8,
    value_msb: u8,
    value_lsb: u8,
) -> Option<[ChannelMessage; 4]> {
    Some([
        ChannelMessageType::ControlChange {
            controller: RPN_MSB,
            value: parameter_number_msb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: RPN_LSB,
            value: parameter_number_lsb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: DATA_ENTRY_MSB,
            value: value_msb,
        }
        .in_channel(channel)?,
        ChannelMessageType::ControlChange {
            controller: DATA_ENTRY_LSB,
            value: value_lsb,
        }
        .in_channel(channel)?,
    ])
}

fn ratio_to_u8s(ratio: Ratio) -> (u8, u8) {
    let as_u16 = (((ratio.as_semitones() + 1.0) * 13f64.exp2()) as u16).clamp(0, 16383);

    ((as_u16 / 128) as u8, (as_u16 % 128) as u8)
}

#[cfg(test)]
mod test {
    use crate::{
        note::{Note, NoteLetter},
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
            let options = ScaleOctaveTuningOptions {
                device_id: 77,
                channels: Channels::Some(channels.iter().cloned().collect()),
                ..Default::default()
            };
            let tuning_message =
                ScaleOctaveTuningMessage::from_octave_tuning(&options, &octave_tuning).unwrap();

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
        let tuning_message =
            ScaleOctaveTuningMessage::from_octave_tuning(&Default::default(), &Default::default())
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
        let kbm = KbmRoot::from(NoteLetter::D.in_octave(4)).to_kbm();
        let tuning = (scl, kbm);

        let options = SingleNoteTuningChangeOptions {
            device_id: 11,
            tuning_program: 22,
            ..Default::default()
        };
        let single_message = SingleNoteTuningChangeMessage::from_tuning(
            &options,
            &tuning,
            (0..127).map(PianoKey::from_midi_number),
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

        let options = SingleNoteTuningChangeOptions {
            device_id: 33,
            tuning_program: 44,
            ..Default::default()
        };

        let split_message = SingleNoteTuningChangeMessage::from_tuning(
            &options,
            &tuning,
            (0..128).map(PianoKey::from_midi_number),
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
            let target_pitch = Note::from_midi_number(0).pitch() * Ratio::from_semitones(target);
            SingleNoteTuningChange { key, target_pitch }
        });

        let tuning_message =
            SingleNoteTuningChangeMessage::from_tuning_changes(&Default::default(), tuning_changes)
                .unwrap();

        assert_eq!(
            Vec::from_iter(tuning_message.sysex_bytes()),
            [[
                0xf0, 0x7f, 0x7f, 0x08, 0x02, 0, 19, 33, 0, 0, 0, 44, 0, 0, 0, 55, 0, 0, 0, 66, 0,
                0, 1, 77, 31, 53, 30, 11, 62, 106, 61, 22, 68, 127, 127, 33, 69, 0, 0, 44, 69, 0,
                0, 55, 69, 0, 0, 66, 69, 0, 1, 77, 69, 32, 0, 11, 69, 63, 127, 22, 69, 64, 0, 33,
                69, 64, 0, 44, 69, 64, 0, 55, 69, 64, 1, 66, 69, 96, 0, 77, 127, 127, 127, 0xf7,
            ]]
        );
        assert_eq!(tuning_message.out_of_range_notes().len(), 4);
    }
}
