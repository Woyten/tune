use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};

use mpsc::SendError;
use oxisynth::{MidiEvent, OxiError, SettingsError, SynthDescriptor, Tuning};
use tune::{
    note::Note,
    pitch::{Pitch, Ratio},
    tuner::{AotTuner, GroupBy, JitTuner, PoolingMode, TunableSynth},
    tuning::KeyboardMapping,
};

pub use oxisynth;
pub use tune;

/// Creates a connected ([`Xenth`], [`AotXenthControl`]) pair.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`AotXenthControl`] part can be used anywhere in your application to control the behavior of the [`Xenth`] part.
pub fn create_aot<K>(
    desc: SynthDescriptor,
    per_semitone_polyphony: u8,
) -> Result<(Xenth, AotXenthControl<K>), SettingsError> {
    let (xenth, tuners) =
        create_internal(desc, per_semitone_polyphony, |synth| AotTuner::start(synth))?;
    Ok((xenth, AotXenthControl { tuners }))
}

/// Creates a connected ([`Xenth`], [`JitXenthControl`]) pair.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`JitXenthControl`] part can be used anywhere in your application to control the behavior of the [`Xenth`] part.
pub fn create_jit<K>(
    desc: SynthDescriptor,
    per_semitone_polyphony: u8,
) -> Result<(Xenth, JitXenthControl<K>), SettingsError> {
    let (xenth, tuners) = create_internal(desc, per_semitone_polyphony, |synth| {
        JitTuner::start(synth, PoolingMode::Stop)
    })?;
    Ok((xenth, JitXenthControl { tuners }))
}

/// Creates a [`Xenth`] instance and several connected [`TunableFluid`] instances.
///
/// The [`Xenth`] part is intended to be used in the audio thread.
/// The [`TunableFluid`] parts can be used anywhere in your application to control the behavior of the [`Xenth`] part.
pub fn create<K>(
    desc: SynthDescriptor,
    per_semitone_polyphony: u8,
) -> Result<(Xenth, Vec<TunableFluid>), SettingsError> {
    let (xenth, tuners) = create_internal(desc, per_semitone_polyphony, |synth| synth)?;
    Ok((xenth, tuners))
}

fn create_internal<T, C: FnMut(TunableFluid) -> T>(
    mut desc: SynthDescriptor,
    polyphony: u8,
    mut xenth_control_creator: C,
) -> Result<(Xenth, Vec<T>), SettingsError> {
    desc.drums_channel_active = false;
    let synth = oxisynth::Synth::new(desc)?;

    let (sender, receiver) = mpsc::channel();

    let tuners = (0..synth.count_midi_channels())
        .collect::<Vec<_>>()
        .chunks_exact(usize::from(polyphony))
        .map(|chunk| {
            xenth_control_creator(TunableFluid {
                sender: sender.clone(),
                offset: chunk[0],
                polyphony: usize::from(polyphony),
            })
        })
        .collect();

    let xenth = Xenth { synth, receiver };

    Ok((xenth, tuners))
}

/// The synthesizing end to be used in the audio thread.
pub struct Xenth {
    synth: oxisynth::Synth,
    receiver: Receiver<Command>,
}

impl Xenth {
    /// Get readable access to the internal [`oxisynth::Synth`] instance in order to query data.
    pub fn synth(&self) -> &oxisynth::Synth {
        &self.synth
    }

    /// Get writeable access to the internal [`oxisynth::Synth`] instance in order to configure its behavior.
    ///
    /// Refrain from modifying the tuning of the internal [`oxisynth::Synth`] instance as `fluid-xenth` will manage the tuning for you.
    pub fn synth_mut(&mut self) -> &mut oxisynth::Synth {
        &mut self.synth
    }

    /// Flushes all commands and uses the internal [`oxisynth::Synth`] instance to create a stream of synthesized audio samples.
    pub fn read(&mut self) -> Result<impl FnMut() -> (f32, f32) + '_, OxiError> {
        for command in self.receiver.try_iter() {
            command(&mut self.synth)?;
        }
        Ok(|| self.synth.read_next())
    }

    /// Flushes all commands and uses the internal [`oxisynth::Synth`] instance to write the synthesized audio using the given `write_callback`.
    pub fn write(
        &mut self,
        len: usize,
        mut write_callback: impl FnMut((f32, f32)),
    ) -> Result<(), OxiError> {
        let mut samples = self.read()?;
        for _ in 0..len {
            write_callback(samples());
        }
        Ok(())
    }
}

/// Controls the connected [`Xenth`] instance from any thread using the ahead-of-time tuning model.
pub struct AotXenthControl<K> {
    tuners: Vec<AotTuner<K, TunableFluid>>,
}

impl<K: Copy + Eq + Hash> AotXenthControl<K> {
    /// Apply the ahead-of-time `tuning` for the given `keys` on the given `xen-channel`.
    pub fn set_tuning(
        &mut self,
        xen_channel: u8,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<usize, SendCommandResult> {
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

    /// Sends a channel-based command to the internal [`oxisynth::Synth`] instance.
    ///
    /// `fluid-xenth` will map the provided `xen_channel` to the internal real channels of the [`oxisynth::Synth`] instance.
    ///
    /// Be aware that calling the "wrong" function (e.g. `add_font`) can put load on the audio thread!
    pub fn send_command(
        &mut self,
        xen_channel: u8,
        command: impl FnMut(&mut oxisynth::Synth, u8) -> Result<(), OxiError> + Send + 'static,
    ) -> SendCommandResult {
        self.get_tuner(xen_channel).global_attr(Box::new(command))
    }

    fn get_tuner(&mut self, xen_channel: u8) -> &mut AotTuner<K, TunableFluid> {
        &mut self.tuners[usize::from(xen_channel)]
    }
}

/// Controls the connected [`Xenth`] instance from any thread using the just-in-time tuning model.
pub struct JitXenthControl<K> {
    tuners: Vec<JitTuner<K, TunableFluid>>,
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

    /// Sends a channel-based command to the internal [`oxisynth::Synth`] instance.
    ///
    /// `fluid-xenth` will map the provided `xen_channel` to the internal real channels of the [`oxisynth::Synth`] instance.
    ///
    /// Be aware that calling the "wrong" function (e.g. `add_font`) can put load on the audio thread!
    pub fn send_command(
        &mut self,
        xen_channel: u8,
        command: impl FnMut(&mut oxisynth::Synth, u8) -> Result<(), OxiError> + Send + 'static,
    ) -> SendCommandResult {
        self.get_tuner(xen_channel).global_attr(Box::new(command))
    }

    fn get_tuner(&mut self, xen_channel: u8) -> &mut JitTuner<K, TunableFluid> {
        &mut self.tuners[usize::from(xen_channel)]
    }
}

/// A [`TunableSynth`] implementation of `fluid-xenth` for later use in an [`AotTuner`] or [`JitTuner`].
pub struct TunableFluid {
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
        let mut detunings = Vec::new();

        for &(detuned_note, detuning) in detuned_notes {
            if let Some(detuned_note) = detuned_note.checked_midi_number() {
                detunings.push((
                    u32::from(detuned_note),
                    Ratio::from_semitones(detuned_note)
                        .stretched_by(detuning)
                        .as_cents(),
                ));
            }
        }

        let mut tuning = Tuning::new(0, 0); // Tuning bank and program have no effect
        self.send_command(move |s| {
            tuning.tune_notes(&detunings).unwrap();
            s.channel_set_tuning(channel, tuning)
        })
    }

    fn note_on(&mut self, channel: usize, started_note: Note, velocity: u8) -> SendCommandResult {
        if let Some(started_note) = started_note.checked_midi_number() {
            let channel = self.get_channel(channel);
            self.send_command(move |s| {
                s.send_event(MidiEvent::NoteOn {
                    channel,
                    key: started_note,
                    vel: velocity,
                })
            })?;
        }
        Ok(())
    }

    fn note_off(&mut self, channel: usize, stopped_note: Note, _velocity: u8) -> SendCommandResult {
        if let Some(stopped_note) = stopped_note.checked_midi_number() {
            let channel = self.get_channel(channel);
            self.send_command(move |s| {
                s.send_event(MidiEvent::NoteOff {
                    channel,
                    key: stopped_note,
                })
            })?;
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
                s.send_event(MidiEvent::PolyphonicKeyPressure {
                    channel,
                    key: affected_note,
                    value: pressure,
                })
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
        command: impl FnOnce(&mut oxisynth::Synth) -> Result<(), OxiError> + Send + 'static,
    ) -> SendCommandResult {
        Ok(self.sender.send(Box::new(command))?)
    }

    fn get_channel(&self, channel: usize) -> u8 {
        u8::try_from(self.offset + channel).unwrap()
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

pub type ChannelCommand = Box<dyn FnMut(&mut oxisynth::Synth, u8) -> Result<(), OxiError> + Send>;

type Command = Box<dyn FnOnce(&mut oxisynth::Synth) -> Result<(), OxiError> + Send>;
