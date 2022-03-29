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
    tuner::{AotTuner, GroupBy, JitTuner, PoolingMode, SetTuningError, TunableSynth},
    tuning::KeyboardMapping,
};

pub use fluidlite;
pub use tune;

const DEFAULT_TUNING_BANK: u32 = 0;

/// Creates a connected ([`Xenth`], [`AotXenthControl`]) pair.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`AotXenthControl`] part can be used anywhere in your application to control the behavior of the [`Xenth`] part.
///
/// Ideally, `synth` is already initialized with all sound fonts loaded. **Important:** The `synth.drums-channel.active` option must be set to `no`!
pub fn create_aot<K>(synth: fluidlite::Synth, polyphony: u8) -> (Xenth, AotXenthControl<K>) {
    let (xenth, tuners, sender) = create(synth, polyphony, |synth| AotTuner::start(synth));
    (xenth, AotXenthControl { tuners, sender })
}

/// Creates a connected ([`Xenth`], [`JitXenthControl`]) pair.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`JitXenthControl`] part can be used anywhere in your application to control the behavior of the [`Xenth`] part.
///
/// Ideally, `synth` is already initialized with all sound fonts loaded. **Important:** The `synth.drums-channel.active` option must be set to `no`!
pub fn create_jit<K>(synth: fluidlite::Synth, polyphony: u8) -> (Xenth, JitXenthControl<K>) {
    let (xenth, tuners, sender) = create(synth, polyphony, |synth| {
        JitTuner::start(synth, PoolingMode::Stop)
    });
    (xenth, JitXenthControl { tuners, sender })
}

fn create<T, C: FnMut(TunableFluid) -> T>(
    synth: fluidlite::Synth,
    polyphony: u8,
    mut xenth_control_creator: C,
) -> (Xenth, Vec<T>, Sender<Command>) {
    let (sender, receiver) = mpsc::channel();

    let num_real_channels = synth.count_midi_channels();

    for channel in 0..num_real_channels {
        synth
            .create_key_tuning(
                DEFAULT_TUNING_BANK,
                channel,
                "fluid-xenth-dynamic-tuning",
                &[0.0; 128],
            )
            .unwrap();
        synth
            .activate_tuning(channel, DEFAULT_TUNING_BANK, channel, true)
            .unwrap();
    }

    let tuners = (0..num_real_channels)
        .collect::<Vec<_>>()
        .chunks_exact(usize::from(polyphony))
        .map(|chunk| {
            xenth_control_creator(TunableFluid {
                sender: sender.clone(),
                offset: usize::try_from(chunk[0]).unwrap(),
                polyphony: usize::from(polyphony),
            })
        })
        .collect();

    let xenth = Xenth { synth, receiver };

    (xenth, tuners, sender)
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

    /// Executes all commands sent by the connected [`JitXenthControl`] instance.
    ///
    /// The use case for this method is to flush non-audio commands (e.g. [`fluidlite::Synth::sfload`]) that can generate load, and, therefore should not be executed in the audio thread.
    pub fn flush_commands(&self) -> fluidlite::Status {
        for command in self.receiver.try_iter() {
            command(&self.synth)?;
        }
        Ok(())
    }
}

/// Controls the connected [`Xenth`] instance from any thread using the ahead-of-time tuning model.
pub struct AotXenthControl<K> {
    tuners: Vec<AotTuner<K, TunableFluid>>,
    sender: Sender<Command>,
}

impl<K: Copy + Eq + Hash> AotXenthControl<K> {
    /// Apply the ahead-of-time `tuning` for the given `keys` on the given `xen-channel`.
    pub fn set_tuning(
        &mut self,
        xen_channel: u8,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<usize, SetTuningError<SendCommandResult>> {
        self.get_tuner(xen_channel).set_tuning(tuning, keys)
    }

    /// Starts a note with a pitch given by the currently loaded tuning on the given `xen_channel`.
    pub fn note_on(&mut self, xen_channel: u8, key: K, velocity: u8) -> SendCommandResult {
        self.get_tuner(xen_channel).note_on(key, velocity)
    }

    /// Stops the note of the given `key` on the given `xen_channel`.
    pub fn note_off(&mut self, xen_channel: u8, key: K) -> SendCommandResult {
        self.get_tuner(xen_channel).note_off(key, 0)
    }

    /// Sends a key-pressure message to the note with the given `key` on the given `xen_channel`.
    pub fn key_pressure(&mut self, xen_channel: u8, key: K, pressure: u8) -> SendCommandResult {
        self.get_tuner(xen_channel).note_attr(key, pressure)
    }

    /// Sends a channel-based command to the internal [`fluidlite::Synth`] instance.
    ///
    /// `fluid-xenth` will map the provided `xen_channel` to the internal real channels of the [`fluidlite::Synth`] instance.
    pub fn send_channel_command(
        &mut self,
        xen_channel: u8,
        command: impl FnMut(&fluidlite::Synth, u32) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        self.get_tuner(xen_channel).global_attr(Box::new(command))
    }

    /// Sends an arbitrary modification command to the internal [`fluidlite::Synth`] instance.
    ///
    /// The command will be executed when [`Xenth::write`] or [`Xenth::flush_commands`] is called.
    /// Be aware that using this method in the wrong way can put load on the audio thread, e.g. when a sound font is loaded.
    ///
    /// Refrain from modifying the tuning of the internal [`fluidlite::Synth`] instance as `fluid-xenth` will manage the tuning for you.
    ///
    /// In order to send channel-based commands use [`AotXenthControl::send_channel_command`].
    pub fn send_command(
        &self,
        command: impl FnOnce(&fluidlite::Synth) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        Ok(self.sender.send(Box::new(command))?)
    }

    fn get_tuner(&mut self, xen_channel: u8) -> &mut AotTuner<K, TunableFluid> {
        &mut self.tuners[usize::from(xen_channel)]
    }
}

/// Controls the connected [`Xenth`] instance from any thread using the just-in-time tuning model.
pub struct JitXenthControl<K> {
    tuners: Vec<JitTuner<K, TunableFluid>>,
    sender: Sender<Command>,
}

impl<K: Copy + Eq + Hash> JitXenthControl<K> {
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
        self.get_tuner(xen_channel).note_on(key, pitch, velocity)
    }

    /// Stops the note of the given `key` on the given `xen_channel`.
    pub fn note_off(&mut self, xen_channel: u8, key: K) -> SendCommandResult {
        self.get_tuner(xen_channel).note_off(key, 0)
    }

    /// Sends a key-pressure message to the note with the given `key` on the given `xen_channel`.
    pub fn key_pressure(&mut self, xen_channel: u8, key: K, pressure: u8) -> SendCommandResult {
        self.get_tuner(xen_channel).note_attr(key, pressure)
    }

    /// Sends a channel-based command to the internal [`fluidlite::Synth`] instance.
    ///
    /// `fluid-xenth` will map the provided `xen_channel` to the internal real channels of the [`fluidlite::Synth`] instance.
    pub fn send_channel_command(
        &mut self,
        xen_channel: u8,
        command: impl FnMut(&fluidlite::Synth, u32) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        self.get_tuner(xen_channel).global_attr(Box::new(command))
    }

    /// Sends an arbitrary modification command to the internal [`fluidlite::Synth`] instance.
    ///
    /// The command will be executed when [`Xenth::write`] or [`Xenth::flush_commands`] is called.
    /// Be aware that using this method in the wrong way can put load on the audio thread, e.g. when a sound font is loaded.
    ///
    /// Refrain from modifying the tuning of the internal [`fluidlite::Synth`] instance as `fluid-xenth` will manage the tuning for you.
    ///
    /// In order to send channel-based commands use [`JitXenthControl::send_channel_command`].
    pub fn send_command(
        &self,
        command: impl FnOnce(&fluidlite::Synth) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        Ok(self.sender.send(Box::new(command))?)
    }

    fn get_tuner(&mut self, xen_channel: u8) -> &mut JitTuner<K, TunableFluid> {
        &mut self.tuners[usize::from(xen_channel)]
    }
}

struct TunableFluid {
    sender: Sender<Command>,
    offset: usize,
    polyphony: usize,
}

impl TunableSynth for TunableFluid {
    type Result = SendCommandResult;
    type NoteAttr = u8;
    type GlobalAttr = ChannelCommand;

    fn num_channels(&self) -> usize {
        self.polyphony
    }

    fn group_by(&self) -> GroupBy {
        GroupBy::Note
    }

    fn notes_detune(
        &mut self,
        channel: usize,
        detuned_notes: &[(Note, Ratio)],
    ) -> SendCommandResult {
        let channel = self.get_channel(channel);
        let mut detuned_keys = Vec::new();
        let mut detunings_in_fluid_format = Vec::new();

        for &(detuned_note, detuning) in detuned_notes {
            if let Some(detuned_note) = detuned_note.checked_midi_number() {
                detuned_keys.push(u32::from(detuned_note));
                detunings_in_fluid_format.push(
                    Ratio::from_semitones(detuned_note)
                        .stretched_by(detuning)
                        .as_cents(),
                );
            }
        }

        self.send_command(move |s| {
            s.tune_notes(
                DEFAULT_TUNING_BANK,
                channel,
                detuned_keys,
                detunings_in_fluid_format,
                true,
            )
        })
    }

    fn note_on(&mut self, channel: usize, started_note: Note, velocity: u8) -> SendCommandResult {
        if let Some(started_note) = started_note.checked_midi_number() {
            let channel = self.get_channel(channel);
            self.send_command(move |s| {
                s.note_on(channel, u32::from(started_note), u32::from(velocity))
            })?;
        }
        Ok(())
    }

    fn note_off(&mut self, channel: usize, stopped_note: Note, _velocity: u8) -> SendCommandResult {
        if let Some(stopped_note) = stopped_note.checked_midi_number() {
            let channel = self.get_channel(channel);
            self.send_command(move |s| s.note_off(channel, u32::from(stopped_note)))?;
        }
        Ok(())
    }

    fn note_attr(
        &mut self,
        channel: usize,
        affected_note: Note,
        pressure: u8,
    ) -> SendCommandResult {
        if let Some(affected_note) = affected_note.checked_midi_number() {
            let channel = self.get_channel(channel);
            self.send_command(move |s| {
                s.key_pressure(channel, u32::from(affected_note), u32::from(pressure))
            })?;
        }
        Ok(())
    }

    fn global_attr(&mut self, mut command: ChannelCommand) -> SendCommandResult {
        let channels = (self.get_channel(0)..).take(self.polyphony);
        self.send_command(move |s| {
            for channel in channels {
                command(s, channel)?;
            }
            Ok(())
        })
    }
}

impl TunableFluid {
    fn send_command(
        &self,
        command: impl FnOnce(&fluidlite::Synth) -> fluidlite::Status + Send + 'static,
    ) -> SendCommandResult {
        Ok(self.sender.send(Box::new(command))?)
    }

    fn get_channel(&self, channel: usize) -> u32 {
        u32::try_from(self.offset + channel).unwrap()
    }
}

pub type SendCommandResult = Result<(), SendCommandError>;

/// Sending a command via [`AotXenthControl`] or [`JitXenthControl`] failed.
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
type ChannelCommand = Box<dyn FnMut(&fluidlite::Synth, u32) -> fluidlite::Status + Send>;
