use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};

use mpsc::SendError;
use tune::{
    note::Note,
    pitch::{Pitch, Ratio},
    tuner::{AccessKeyResult, GroupByNote, JitTuner, PoolingMode, RegisterKeyResult},
};

/// Creates a connected ([`Xenth`], [`XenthControl`]) pair.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`XenthControl`] part can be used anywhere in your application to control the behavior of the [`Xenth`] part.
///
/// Ideally, `synth` is already initialized with all sound fonts loaded. **Important:** The `synth.drums-channel.active` option must be set to `no`!
pub fn create<K: Copy + Eq + Hash>(
    synth: fluidlite::Synth,
    polyphony: u8,
) -> (Xenth, XenthControl<K>) {
    let num_real_channels = synth.count_midi_channels();

    for channel in 0..num_real_channels {
        // Since tunings are not applied in real-time, it is okay to use a single tuning bank (127) and tuning program (127)
        synth
            .create_key_tuning(127, 127, "fluid-xenth-dynamic-tuning", &[0.0; 128])
            .unwrap();
        synth.activate_tuning(channel, 127, 127, true).unwrap();
    }

    let tuners = (0..(num_real_channels / u32::from(polyphony)))
        .map(|_| JitTuner::new(GroupByNote, PoolingMode::Stop, usize::from(polyphony)))
        .collect();

    let (sender, receiver) = mpsc::channel();

    let xenth = Xenth { synth, receiver };

    let xenth_control = XenthControl {
        tuners,
        polyphony,
        sender,
    };

    (xenth, xenth_control)
}

/// The synthesizing end to be used in the audio thread.
pub struct Xenth {
    synth: fluidlite::Synth,
    receiver: Receiver<Command>,
}

impl Xenth {
    /// Calls [`Xenth::flush_commands`] and uses the internal [`fluidlite::Synth`] instance to write the synthesized audio signal to `audio_buffer`.
    pub fn write<S: fluidlite::IsSamples>(&self, audio_buffer: S) -> fluidlite::Status {
        self.flush_commands()?;
        self.synth.write(audio_buffer)
    }

    /// Executes all commands sent by the connected [`XenthControl`] instance.
    ///
    /// The use case for this method is to flush non-audio commands (e.g. [`fluidlite::Synth::sfload`]) that can generate load, and, therefore should not be executed in the audio thread.
    pub fn flush_commands(&self) -> fluidlite::Status {
        for command in self.receiver.try_iter() {
            command(&self.synth)?;
        }
        Ok(())
    }
}

/// Controls the connected [`Xenth`] instance from any thread.
pub struct XenthControl<K> {
    tuners: Vec<JitTuner<K, GroupByNote>>,
    polyphony: u8,
    sender: Sender<Command>,
}

impl<K: Copy + Eq + Hash> XenthControl<K> {
    /// Starts a note with the given `pitch` on the given `xen_channel`.
    ///
    /// `key` is used as identifier for currently sounding notes.
    pub fn note_on(
        &mut self,
        xen_channel: u8,
        key: K,
        pitch: Pitch,
        velocity: u8,
    ) -> SendCommandResult {
        let offset = usize::from(xen_channel) * usize::from(self.polyphony);
        match self.tuners[usize::from(xen_channel)].register_key(key, pitch) {
            RegisterKeyResult::Accepted {
                channel,
                stopped_note,
                started_note,
                detuning,
            } => {
                if let Some(stopped_note) = stopped_note.and_then(Note::checked_midi_number) {
                    self.send_command(move |s| {
                        s.note_off(
                            u32::try_from(channel + offset).unwrap(),
                            u32::from(stopped_note),
                        )
                    })?;
                }
                if let Some(started_note) = started_note.checked_midi_number() {
                    let detuning_in_fluid_format =
                        (Ratio::from_semitones(started_note).stretched_by(detuning)).as_cents();

                    self.send_command(move |s| {
                        s.tune_notes(
                            127,
                            127,
                            [u32::from(started_note)],
                            [detuning_in_fluid_format],
                            true,
                        )?;
                        s.note_on(
                            u32::try_from(channel + offset).unwrap(),
                            u32::from(started_note),
                            u32::from(velocity),
                        )?;
                        Ok(())
                    })?;
                }
            }
            RegisterKeyResult::Rejected => {}
        }
        Ok(())
    }

    /// Stops the note of the given `key` on the given `xen_channel`.
    pub fn note_off(&mut self, xen_channel: u8, key: &K) -> SendCommandResult {
        let offset = usize::from(xen_channel) * usize::from(self.polyphony);
        match self.tuners[usize::from(xen_channel)].deregister_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.send_command(move |s| {
                        // When note_on is called for a note that is not supported by the current sound program FluidLite ignores that call.
                        // When note_off is sent for the same note afterwards FluidLite reports an error since the note is considered off.
                        // This error cannot be anticipated so we just ignore it.

                        let _ = s.note_off(
                            u32::try_from(channel + offset).unwrap(),
                            u32::from(found_note),
                        );
                        Ok(())
                    })?;
                }
            }
            AccessKeyResult::NotFound => {}
        }
        Ok(())
    }

    /// Sends a key-pressure message to the note with the given `key` on the given `xen_channel`.
    pub fn key_pressure(&self, xen_channel: u8, key: &K, pressure: u8) -> SendCommandResult {
        let offset = usize::from(xen_channel) * usize::from(self.polyphony);
        match self.tuners[usize::from(xen_channel)].access_key(key) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.send_command(move |s| {
                        s.key_pressure(
                            u32::try_from(channel + offset).unwrap(),
                            u32::from(found_note),
                            u32::from(pressure),
                        )
                    })?;
                }
            }
            AccessKeyResult::NotFound => {}
        }
        Ok(())
    }

    /// Sends an arbitrary modification command to the internal [`fluidlite::Synth`] instance.
    ///
    /// The command will be executed when [`Xenth::write`] or [`Xenth::flush_commands`] is called.
    /// Be aware that using this method in the wrong way can put load on the audio thread, e.g. when a sound font is loaded.
    ///
    /// Refrain from modifying the tuning of the internal [`fluidlite::Synth`] instance as `fluid-xenth` will manage the tuning for you.
    ///
    /// In order to send channel-based commands use [`XenthControl::send_channel_command`].
    pub fn send_command(
        &self,
        command: impl FnOnce(&fluidlite::Synth) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        Ok(self.sender.send(Box::new(command))?)
    }

    /// Sends a channel-based command to the internal [`fluidlite::Synth`] instance.
    ///
    /// `fluid-xenth` will map the provided `xen_channel` to the internal real channels of the [`fluidlite::Synth`] instance.
    pub fn send_channel_command(
        &self,
        xen_channel: u8,
        mut command: impl FnMut(&fluidlite::Synth, u32) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        let real_channels = (xen_channel * self.polyphony)..(xen_channel + 1) * self.polyphony;
        self.send_command(move |s| {
            for channel in real_channels {
                command(s, u32::from(channel))?;
            }
            Ok(())
        })
    }
}

pub type SendCommandResult = Result<(), SendCommandError>;

/// Sending a command via [`XenthControl`] failed.
///
/// This error can only occur if the receiving [`Xenth`] instance has been disconnected / torn down.
#[derive(Copy, Clone, Debug)]
pub struct SendCommandError;

impl Display for SendCommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The receiving Xenth instance has been torn down. Is the audio thread still alive?"
        )
    }
}

impl<T> From<SendError<T>> for SendCommandError {
    fn from(_: SendError<T>) -> Self {
        SendCommandError
    }
}

impl Error for SendCommandError {}

type Command = Box<dyn FnOnce(&fluidlite::Synth) -> fluidlite::Status + Send>;
