//! Basic abstractions for MIDI Channel Voice / Channel Mode messages.
//!
//! References:
//! - [MIDI messages](https://www.midi.org/specifications-old/item/table-1-summary-of-midi-message)

use std::convert::TryFrom;

use crate::{key::PianoKey, tuner::ChannelTuner};

/// Status byte for "Note Off event".
pub const NOTE_OFF: u8 = 0b1000;
/// Status byte for "Note On event".
pub const NOTE_ON: u8 = 0b1001;
/// Status byte for "Polyphonic Key Pressure (Aftertouch)".
pub const POLYPHONIC_KEY_PRESSURE: u8 = 0b1010;
/// Status byte for "Control Change".
pub const CONTROL_CHANGE: u8 = 0b1011;
/// Status byte for "Program Change".
pub const PROGRAM_CHANGE: u8 = 0b1100;
/// Status byte for "Channel Pressure (After-touch)".
pub const CHANNEL_PRESSURE: u8 = 0b1101;
/// Status byte for "Channel Pressure (After-touch)".
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
                value: u32::from(*message.get(1)?) + u32::from(*message.get(2)?) * 128,
            },
            _ => return None,
        };
        Some(ChannelMessage {
            channel,
            message_type,
        })
    }

    pub fn channel(&self) -> u8 {
        self.channel
    }

    pub fn message_type(&self) -> ChannelMessageType {
        self.message_type
    }

    /// Distributes the given MIDI message to multiple channels depending on the state of the provided [`ChannelTuner`].
    pub fn distribute(&self, tuner: &ChannelTuner, channel_offset: u8) -> Vec<[u8; 3]> {
        match self.message_type {
            ChannelMessageType::NoteOff { key, velocity } => {
                polyphonic_channel_message(tuner, channel_offset, NOTE_OFF, key, velocity)
            }
            ChannelMessageType::NoteOn { key, velocity } => {
                polyphonic_channel_message(tuner, channel_offset, NOTE_ON, key, velocity)
            }
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                polyphonic_channel_message(
                    tuner,
                    channel_offset,
                    POLYPHONIC_KEY_PRESSURE,
                    key,
                    pressure,
                )
            }
            ChannelMessageType::ControlChange { controller, value } => {
                monophonic_channel_message(channel_offset, CONTROL_CHANGE, controller, value)
            }
            ChannelMessageType::ProgramChange { program } => {
                monophonic_channel_message(channel_offset, PROGRAM_CHANGE, program, 0)
            }
            ChannelMessageType::ChannelPressure { pressure } => {
                monophonic_channel_message(channel_offset, CHANNEL_PRESSURE, pressure, 0)
            }
            ChannelMessageType::PitchBendChange { value } => monophonic_channel_message(
                channel_offset,
                PITCH_BEND_CHANGE,
                (value % 128) as u8,
                (value / 128) as u8,
            ),
        }
    }
}

fn polyphonic_channel_message(
    tuner: &ChannelTuner,
    channel_offset: u8,
    prefix: u8,
    key: u8,
    payload: u8,
) -> Vec<[u8; 3]> {
    if let Some((channel, note)) =
        tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(key.into()))
    {
        if let (Ok(channel), Ok(note)) = (
            u8::try_from(channel + usize::from(channel_offset)),
            u8::try_from(note.midi_number()),
        ) {
            if channel < 16 && note < 128 {
                return vec![[prefix << 4 | channel, note, payload]];
            }
        }
    }
    return vec![];
}

fn monophonic_channel_message(
    channel_offset: u8,
    prefix: u8,
    payload1: u8,
    payload2: u8,
) -> Vec<[u8; 3]> {
    (channel_offset..16)
        .map(|channel| [prefix << 4 | channel, payload1, payload2])
        .collect()
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
    PitchBendChange { value: u32 },
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
}
