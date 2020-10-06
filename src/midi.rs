//! Basic abstractions for MIDI Channel Voice / Channel Mode messages.
//!
//! References:
//! - [MIDI messages](https://www.midi.org/specifications-old/item/table-1-summary-of-midi-message)

use crate::{key::PianoKey, ratio::Ratio, tuner::ChannelTuner, tuning::Tuning};
use std::convert::{TryFrom, TryInto};

/// Status bits for "Note Off event".
pub const NOTE_OFF: u8 = 0b1000;
/// Status bits for "Note On event".
pub const NOTE_ON: u8 = 0b1001;
/// Status bits for "Polyphonic Key Pressure (Aftertouch)".
pub const POLYPHONIC_KEY_PRESSURE: u8 = 0b1010;
/// Status bits for "Control Change".
pub const CONTROL_CHANGE: u8 = 0b1011;
/// Status bits for "Program Change".
pub const PROGRAM_CHANGE: u8 = 0b1100;
/// Status bits for "Channel Pressure (After-touch)".
pub const CHANNEL_PRESSURE: u8 = 0b1101;
/// Status bits for "Channel Pressure (After-touch)".
pub const PITCH_BEND_CHANGE: u8 = 0b1110;

/// A type-safe representation of MIDI messages that aren't System Common messages.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ChannelMessage {
    channel: u8,
    message_type: ChannelMessageType,
}

impl ChannelMessage {
    /// Parses a MIDI message.
    ///
    /// When no valid Channel Voice or Channel Mode message is provided [`None`] is returned.
    ///
    /// # Examples
    /// ```
    /// # use tune::midi::ChannelMessage;
    /// # use tune::midi::ChannelMessageType;
    /// let message = ChannelMessage::from_raw_message(&[0b1001_1000, 77, 88]).unwrap();
    /// assert_eq!(message.channel(), 8);
    /// assert_eq!(
    ///     message.message_type(),
    ///     ChannelMessageType::NoteOn {
    ///         key: 77,
    ///         velocity: 88
    ///     }
    /// );
    ///
    /// let invalid_message = [1, 2, 3];
    /// assert_eq!(ChannelMessage::from_raw_message(&invalid_message), None);
    /// ```
    pub fn from_raw_message(message: &[u8]) -> Option<ChannelMessage> {
        let status_byte = *message.get(0)?;
        let channel = status_byte & 0b0000_1111;
        let action = status_byte >> 4;
        let message_type = match action {
            NOTE_OFF => ChannelMessageType::NoteOff {
                key: *message.get(1)?,
                velocity: *message.get(2)?,
            },
            NOTE_ON => ChannelMessageType::NoteOn {
                key: *message.get(1)?,
                velocity: *message.get(2)?,
            },
            POLYPHONIC_KEY_PRESSURE => ChannelMessageType::PolyphonicKeyPressure {
                key: *message.get(1)?,
                pressure: *message.get(2)?,
            },
            CONTROL_CHANGE => ChannelMessageType::ControlChange {
                controller: *message.get(1)?,
                value: *message.get(2)?,
            },
            PROGRAM_CHANGE => ChannelMessageType::ProgramChange {
                program: *message.get(1)?,
            },
            CHANNEL_PRESSURE => ChannelMessageType::ChannelPressure {
                pressure: *message.get(1)?,
            },
            PITCH_BEND_CHANGE => ChannelMessageType::PitchBendChange {
                value: u16::from(*message.get(1)?) + u16::from(*message.get(2)?) * 128,
            },
            _ => return None,
        };
        Some(ChannelMessage {
            channel,
            message_type,
        })
    }

    /// Returns the byte representation of a MIDI message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::midi::ChannelMessageType;
    /// let message = ChannelMessageType::NoteOn {
    ///         key: 77,
    ///         velocity: 88
    ///     }
    ///     .in_channel(7)
    ///     .unwrap();
    ///
    /// assert_eq!(message.to_raw_message(), [0b1001_0111, 77, 88]);
    /// ```
    pub fn to_raw_message(&self) -> [u8; 3] {
        match self.message_type {
            ChannelMessageType::NoteOff { key, velocity } => {
                channel_message(NOTE_OFF, self.channel, key, velocity)
            }
            ChannelMessageType::NoteOn { key, velocity } => {
                channel_message(NOTE_ON, self.channel, key, velocity)
            }
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                channel_message(POLYPHONIC_KEY_PRESSURE, self.channel, key, pressure)
            }
            ChannelMessageType::ControlChange { controller, value } => {
                channel_message(CONTROL_CHANGE, self.channel, controller, value)
            }
            ChannelMessageType::ProgramChange { program } => {
                channel_message(PROGRAM_CHANGE, self.channel, program, 0)
            }
            ChannelMessageType::ChannelPressure { pressure } => {
                channel_message(CHANNEL_PRESSURE, self.channel, pressure, 0)
            }
            ChannelMessageType::PitchBendChange { value } => channel_message(
                PITCH_BEND_CHANGE,
                self.channel,
                (value % 128) as u8,
                (value / 128) as u8,
            ),
        }
    }

    /// Returns the channel of a MIDI message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::midi::ChannelMessage;
    /// # use tune::midi::ChannelMessageType;
    /// let message = ChannelMessage::from_raw_message(&[0b1001_1000, 77, 88]).unwrap();
    /// assert_eq!(message.channel(), 8);
    /// ```
    pub fn channel(&self) -> u8 {
        self.channel
    }

    /// Returns the channel-agnostic part of a MIDI message.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::midi::ChannelMessage;
    /// # use tune::midi::ChannelMessageType;
    /// let message = ChannelMessage::from_raw_message(&[0b1001_1000, 77, 88]).unwrap();
    /// assert!(matches!(message.message_type(), ChannelMessageType::NoteOn { .. }));
    /// ```
    pub fn message_type(&self) -> ChannelMessageType {
        self.message_type
    }

    /// Applies a tuning transformation to a MIDI message.
    ///
    /// This operation only succeeds for polyphonic messages whose transformed note is in the allowed MIDI range [0..128).
    /// If the transformation is successful the deviation from the accurate transformed note is reported.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::midi::ChannelMessageType;
    /// # use tune::midi::TransformResult;
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// # use tune::scala::Scl;
    /// let tuning = (
    ///     Scl::builder().push_cents(120.0).build().unwrap(),
    ///     Kbm::root_at(Note::from_midi_number(62))
    /// );
    ///
    /// // Usually, polyphonic messages are transformed
    ///
    /// let in_range = ChannelMessageType::NoteOn { key: 100, velocity: 88 }
    ///     .in_channel(8)
    ///     .unwrap();
    ///
    /// match in_range.transform(&tuning) {
    ///     TransformResult::Transformed { message, note, deviation } => {
    ///         assert_eq!(message.channel(), 8);
    ///         assert_eq!(
    ///             message.message_type(),
    ///             ChannelMessageType::NoteOn { key: 108, velocity: 88 }
    ///         );
    ///         assert_eq!(note, 108);
    ///         assert_approx_eq!(deviation.as_cents(), -40.0);
    ///     },
    ///     _ => unreachable!(),
    /// }
    ///
    /// // When the transformed note is out of range messages are not transformed
    ///
    /// let out_of_range = ChannelMessageType::NoteOn { key: 120, velocity: 88 }
    ///     .in_channel(8)
    ///     .unwrap();
    ///
    /// assert!(matches!(out_of_range.transform(&tuning), TransformResult::NoteOutOfRange));
    ///
    /// // Monophonic messages are never transformed
    ///
    /// let not_transformed = ChannelMessageType::ProgramChange { program: 42 }
    ///     .in_channel(8)
    ///     .unwrap();
    ///
    /// assert!(matches!(not_transformed.transform(&tuning), TransformResult::NotKeyBased));
    /// ```
    pub fn transform(&self, tuning: &impl Tuning<PianoKey>) -> TransformResult {
        let mut cloned = *self;

        match cloned.message_type.get_key_mut() {
            Some(key) => {
                let piano_key = PianoKey::from_midi_number(*key);
                let pitch = tuning.pitch_of(piano_key);
                let approximation = pitch.find_in(&());

                match u8::try_from(approximation.approx_value.midi_number()) {
                    Ok(note) if note < 128 => {
                        *key = note;
                        TransformResult::Transformed {
                            message: cloned,
                            note,
                            deviation: approximation.deviation,
                        }
                    }
                    _ => TransformResult::NoteOutOfRange,
                }
            }
            None => TransformResult::NotKeyBased,
        }
    }
}

fn channel_message(prefix: u8, channel: u8, payload1: u8, payload2: u8) -> [u8; 3] {
    [prefix << 4 | channel, payload1, payload2]
}

/// The result of a tuning transformation on a MIDI message.
#[derive(Copy, Clone, Debug)]
pub enum TransformResult {
    Transformed {
        message: ChannelMessage,
        note: u8,
        deviation: Ratio,
    },
    NotKeyBased,
    NoteOutOfRange,
}

/// A parsed representation of the channel-agnostic part of a MIDI message.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ChannelMessageType {
    NoteOff { key: u8, velocity: u8 },
    NoteOn { key: u8, velocity: u8 },
    PolyphonicKeyPressure { key: u8, pressure: u8 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
    ChannelPressure { pressure: u8 },
    PitchBendChange { value: u16 },
}

impl ChannelMessageType {
    /// Creates a new [`ChannelMessage`] from `self` with the given `channel`.
    ///
    /// [`None`] is returned if the channel value is outside the range [0..16).
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::midi::ChannelMessageType;
    /// let message_type = ChannelMessageType::NoteOn {
    ///         key: 77,
    ///         velocity: 88
    ///     };
    /// let message = message_type.in_channel(15).unwrap();
    ///
    /// assert_eq!(message.channel(), 15);
    /// assert_eq!(message.message_type(), message_type);
    ///
    /// let channel_out_of_range = message_type.in_channel(16);
    /// assert!(channel_out_of_range.is_none());
    /// ```
    pub fn in_channel(self, channel: u8) -> Option<ChannelMessage> {
        match channel < 16 {
            true => Some(ChannelMessage {
                channel,
                message_type: self,
            }),
            false => None,
        }
    }

    /// Distributes the given [`ChannelMessageType`] to multiple channels depending on the state of the provided [`ChannelTuner`].
    ///
    /// The parameter `channel_offset` specifies the amount by which the channel number returned by `tuner` will be raised.
    /// Messages that cannot be mapped to a valid MIDI message will be discarded.
    ///
    /// # Examples
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::midi::ChannelMessageType;
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// # use tune::scala::Scl;
    /// # use tune::tuner::ChannelTuner;
    /// let mut tuning = (
    ///     Scl::builder().push_cents(25.0).build().unwrap(),
    ///     Kbm::root_at(Note::from_midi_number(62)),
    /// );
    ///
    /// let mut tuner = ChannelTuner::new();
    /// tuner.apply_full_keyboard_tuning(
    ///     &tuning,
    ///     PianoKey::from_midi_number(0),
    ///     PianoKey::from_midi_number(128),
    /// );
    ///
    /// // Usually, polyponic messages are distributed
    ///
    /// let in_range = ChannelMessageType::NoteOn {
    ///     key: 90,
    ///     velocity: 88,
    /// };
    ///
    /// let distributed = in_range.distribute(&tuner, 4);
    /// assert_eq!(distributed.len(), 1);
    /// assert_eq!(
    ///     distributed[0].message_type(),
    ///     ChannelMessageType::NoteOn {
    ///         key: 69,
    ///         velocity: 88,
    ///     }
    /// );
    /// assert_eq!(distributed[0].channel(), 6);
    ///
    /// // When mapped channel is out of range messages are discarded
    ///
    /// assert!(in_range.distribute(&tuner, 14).is_empty());
    ///
    /// // When transformed note is out of range messages are discarded
    ///
    /// let mut macrotuning = (
    ///     Scl::builder().push_cents(120.0).build().unwrap(),
    ///     Kbm::root_at(Note::from_midi_number(62)),
    /// );
    ///
    /// let mut macrotuner = ChannelTuner::new();
    /// macrotuner.apply_full_keyboard_tuning(
    ///     &macrotuning,
    ///     PianoKey::from_midi_number(0),
    ///     PianoKey::from_midi_number(128),
    /// );
    ///
    /// let out_of_range = ChannelMessageType::NoteOn {
    ///     key: 120,
    ///     velocity: 88,
    /// };
    ///
    /// assert!(out_of_range.distribute(&macrotuner, 4).is_empty());
    ///
    /// // Monophonic messages are distributed to multiple channels
    ///
    /// let monophonic = ChannelMessageType::ProgramChange { program: 42 };
    ///
    /// let distributed = monophonic.distribute(&tuner, 4);
    /// assert_eq!(distributed.len(), 4);
    /// for (index, channel) in (0..4).zip(4..8) {
    ///     assert_eq!(
    ///         distributed[index].message_type(),
    ///         ChannelMessageType::ProgramChange { program: 42 }
    ///     );
    ///     assert_eq!(distributed[index].channel(), channel);
    /// }
    ///
    /// // When mapped channel is out of range messages are discarded
    ///
    /// let distributed = monophonic.distribute(&tuner, 14);
    /// assert_eq!(distributed.len(), 2);
    /// for (index, channel) in (0..2).zip(14..16) {
    ///     assert_eq!(
    ///         distributed[index].message_type(),
    ///         ChannelMessageType::ProgramChange { program: 42 }
    ///     );
    ///     assert_eq!(distributed[index].channel(), channel);
    /// }
    /// ```
    pub fn distribute(&self, tuner: &ChannelTuner, channel_offset: u8) -> Vec<ChannelMessage> {
        let mut cloned = *self;

        match cloned.get_key_mut() {
            Some(key) => {
                if let Some((channel, note)) =
                    tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(*key))
                {
                    if let (Ok(channel), Ok(note)) = (
                        u8::try_from(channel + usize::from(channel_offset)),
                        u8::try_from(note.midi_number()),
                    ) {
                        if channel < 16 && note < 128 {
                            *key = note;
                            return vec![ChannelMessage {
                                channel,
                                message_type: cloned,
                            }];
                        }
                    }
                }
                vec![]
            }
            None => (channel_offset
                ..(usize::from(channel_offset) + tuner.num_channels())
                    .min(16)
                    .try_into()
                    .unwrap())
                .map(|channel| ChannelMessage {
                    channel,
                    message_type: *self,
                })
                .collect(),
        }
    }

    fn get_key_mut(&mut self) -> Option<&mut u8> {
        match self {
            ChannelMessageType::NoteOff { key, .. } => Some(key),
            ChannelMessageType::NoteOn { key, .. } => Some(key),
            ChannelMessageType::PolyphonicKeyPressure { key, .. } => Some(key),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_note_off() {
        let message = ChannelMessage::from_raw_message(&[0b1000_0111, 88, 99]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 7,
                message_type: ChannelMessageType::NoteOff {
                    key: 88,
                    velocity: 99
                }
            }
        );
    }

    #[test]
    fn parse_note_on() {
        let message = ChannelMessage::from_raw_message(&[0b1001_1000, 77, 88]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 8,
                message_type: ChannelMessageType::NoteOn {
                    key: 77,
                    velocity: 88
                }
            }
        );
    }

    #[test]
    fn parse_polyphonic_key_pressure() {
        let message = ChannelMessage::from_raw_message(&[0b1010_1001, 66, 77]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 9,
                message_type: ChannelMessageType::PolyphonicKeyPressure {
                    key: 66,
                    pressure: 77
                }
            }
        );
    }

    #[test]
    fn parse_control_change() {
        let message = ChannelMessage::from_raw_message(&[0b1011_1010, 55, 66]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 10,
                message_type: ChannelMessageType::ControlChange {
                    controller: 55,
                    value: 66
                }
            }
        );
    }

    #[test]
    fn parse_program_change() {
        let message = ChannelMessage::from_raw_message(&[0b1100_1011, 44]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 11,
                message_type: ChannelMessageType::ProgramChange { program: 44 }
            }
        );
    }

    #[test]
    fn parse_channel_pressure() {
        let message = ChannelMessage::from_raw_message(&[0b1101_1100, 33]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 12,
                message_type: ChannelMessageType::ChannelPressure { pressure: 33 }
            }
        );
    }

    #[test]
    fn parse_pitch_bend_change() {
        let message = ChannelMessage::from_raw_message(&[0b1110_1101, 22, 33]).unwrap();
        assert_eq!(
            message,
            ChannelMessage {
                channel: 13,
                message_type: ChannelMessageType::PitchBendChange { value: 4246 }
            }
        );
    }

    #[test]
    fn serialize_note_off() {
        let message = ChannelMessage {
            channel: 7,
            message_type: ChannelMessageType::NoteOff {
                key: 88,
                velocity: 99,
            },
        };

        assert_eq!(message.to_raw_message(), [0b1000_0111, 88, 99]);
    }

    #[test]
    fn serialize_note_on() {
        let message = ChannelMessage {
            channel: 8,
            message_type: ChannelMessageType::NoteOn {
                key: 77,
                velocity: 88,
            },
        };
        assert_eq!(message.to_raw_message(), [0b1001_1000, 77, 88]);
    }

    #[test]
    fn serialize_polyphonic_key_pressure() {
        let message = ChannelMessage {
            channel: 9,
            message_type: ChannelMessageType::PolyphonicKeyPressure {
                key: 66,
                pressure: 77,
            },
        };
        assert_eq!(message.to_raw_message(), [0b1010_1001, 66, 77]);
    }

    #[test]
    fn serialize_control_change() {
        let message = ChannelMessage {
            channel: 10,
            message_type: ChannelMessageType::ControlChange {
                controller: 55,
                value: 66,
            },
        };
        assert_eq!(message.to_raw_message(), [0b1011_1010, 55, 66]);
    }

    #[test]
    fn serialize_program_change() {
        let message = ChannelMessage {
            channel: 11,
            message_type: ChannelMessageType::ProgramChange { program: 44 },
        };
        assert_eq!(message.to_raw_message(), [0b1100_1011, 44, 0]);
    }

    #[test]
    fn serialize_channel_pressure() {
        let message = ChannelMessage {
            channel: 12,
            message_type: ChannelMessageType::ChannelPressure { pressure: 33 },
        };
        assert_eq!(message.to_raw_message(), [0b1101_1100, 33, 0]);
    }

    #[test]
    fn serialize_pitch_bend_change() {
        let message = ChannelMessage {
            channel: 13,
            message_type: ChannelMessageType::PitchBendChange { value: 4246 },
        };
        assert_eq!(message.to_raw_message(), [0b1110_1101, 22, 33]);
    }
}
