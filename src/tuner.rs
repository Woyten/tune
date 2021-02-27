//! Generate tuning maps to enhance the capabilities of synthesizers with limited tuning support.

use std::{collections::HashMap, hash::Hash};

use crate::{
    mts::{
        Channels, DeviceId, ScaleOctaveTuning, ScaleOctaveTuningError, ScaleOctaveTuningMessage,
        SingleNoteTuningChange, SingleNoteTuningChangeError, SingleNoteTuningChangeMessage,
    },
    note::{Note, NoteLetter},
    pitch::{Pitched, Ratio},
    tuning::KeyboardMapping,
};

/// Maps keys accross multiple channels to overcome several tuning limitations.
pub struct ChannelTuner<K> {
    key_map: HashMap<K, (usize, Note)>,
    num_channels: usize,
}

impl<K: Copy + Eq + Hash> ChannelTuner<K> {
    pub fn empty() -> Self {
        Self {
            key_map: HashMap::new(),
            num_channels: 0,
        }
    }

    /// Distributes the provided [`KeyboardMapping`] accross multiple channels s.t. each note is only detuned once per channel and by 50c at most.
    ///
    /// This works around a restriction of some synthesizers (e.g. FluidSynth) where the pitch per note can be customized but the sound sample per note cannot.
    ///
    /// Apply this strategy if your synthesizer has full keyboard tuning support but your samples sound as if they were played back in slow motion or time lapse at certain pitches.
    ///
    /// # Examples
    ///
    /// In the following example, `tuner` holds a [`ChannelTuner`] instance which encapsulates the mapping required to find the appropriate channel and note for a given scale degree.
    /// The variable `channel_tunings` stores a `Vec` of tunings that need to be applied on the channels of your synthesizer.
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::KbmRoot;
    /// # use tune::scala::Scl;
    /// # use tune::tuner::ChannelTuner;
    /// let scl = Scl::builder()
    ///     .push_ratio(Ratio::octave().divided_into_equal_steps(36))
    ///     .build()
    ///     .unwrap();
    ///
    /// let edo_36_tuning = (scl, KbmRoot::from(Note::from_midi_number(62)).to_kbm());
    ///
    /// let (tuner, channel_tunings) = ChannelTuner::apply_full_keyboard_tuning(
    ///     edo_36_tuning,
    ///     (0..128).map(PianoKey::from_midi_number),
    /// );
    ///
    /// // Since 3 36-EDO notes fit into one semitone, 3 channels are required.
    /// assert_eq!(tuner.num_channels(), 3);
    /// assert_eq!(tuner.num_channels(), channel_tunings.len());
    ///
    /// assert_eq!(
    ///     tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(60)),
    ///     Some((2, Note::from_midi_number(61)))
    /// );
    /// assert_eq!(
    ///     tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(61)),
    ///     Some((0, Note::from_midi_number(62)))
    /// );
    /// assert_eq!(
    ///     tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(62)),
    ///     Some((1, Note::from_midi_number(62)))
    /// );
    /// assert_eq!(
    ///     tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(63)),
    ///     Some((2, Note::from_midi_number(62)))
    /// );
    /// assert_eq!(
    ///     tuner.get_channel_and_note_for_key(PianoKey::from_midi_number(64)),
    ///     Some((0, Note::from_midi_number(63)))
    /// );
    /// ```
    pub fn apply_full_keyboard_tuning(
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> (Self, Vec<FullKeyboardDetuning>) {
        Self::apply_tuning_internal(
            tuning,
            keys,
            |note| note,
            |tuning_map| FullKeyboardDetuning { tuning_map },
        )
    }

    /// Distributes the provided [`KeyboardMapping`] accross multiple channels s.t. each note *letter* is only detuned once per channel and by 50c at most.
    ///
    /// This method works in the same way as [`ChannelTuner::apply_full_keyboard_tuning`] does but instead of retuning each note individually, the retuning pattern repeats at the octave.
    ///
    /// When applied to octave-repeating scales the octave-based tuning strategy and the full keyboard tuning strategy work equally well.
    /// For non-octave-repeating scales, however, the situation is different:
    /// Since only few (if any) notes can share the same detuning in different octaves the octave-based tuning strategy will require a large number of channels to account for all items of a tuning.
    ///
    /// Apply this strategy if your synthesizer supports octave-based tunings but does not support full keyboard tunings.
    pub fn apply_octave_based_tuning(
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> (Self, Vec<OctaveBasedDetuning>) {
        Self::apply_tuning_internal(
            tuning,
            keys,
            |note| note.letter_and_octave().0,
            |tuning_map| OctaveBasedDetuning { tuning_map },
        )
    }

    /// Distributes the provided [`KeyboardMapping`] accross multiple channels where each channel is detuned as a whole and by 50c at most.
    ///
    /// This tuning method is the least powerful one and should only be used if your synthesizer has neither full keyboard nor octave-based tuning support.
    /// It works quite well for *n*-edo tunings where gcd(*n*,&nbsp;12) is large.
    /// This because each channel can handle gcd(*n*,&nbsp;12) notes resulting in a total number of required channels of *n*&nbsp;/&nbsp;gcd(*n*,&nbsp;12).
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::KbmRoot;
    /// # use tune::scala::Scl;
    /// # use tune::tuner::ChannelTuner;
    /// let kbm = KbmRoot::from(Note::from_midi_number(62)).to_kbm();
    ///
    /// let scl_of_16_edo = Scl::builder()
    ///     .push_ratio(Ratio::octave().divided_into_equal_steps(16))
    ///     .build()
    ///     .unwrap();
    ///
    /// let (_, tunings) = ChannelTuner::apply_channel_based_tuning(
    ///     (scl_of_16_edo, &kbm),
    ///     (0..128).map(PianoKey::from_midi_number),
    /// );
    ///
    /// // The number of channels for 16-edo is 4 = 16/gcd(16, 12)
    /// assert_eq!(tunings.len(), 4);
    /// assert_approx_eq!(tunings[0].as_cents(), -25.0);
    /// assert_approx_eq!(tunings[1].as_cents(), 0.0);
    /// assert_approx_eq!(tunings[2].as_cents(), 25.0);
    /// assert_approx_eq!(tunings[3].as_cents(), 50.0);
    ///
    /// let scl_of_13_edt = Scl::builder()
    ///     .push_ratio(Ratio::from_float(3.0).divided_into_equal_steps(13))
    ///     .build()
    ///     .unwrap();
    ///
    /// let (_, tunings) = ChannelTuner::apply_channel_based_tuning(
    ///     (scl_of_13_edt, &kbm),
    ///     (0..128).map(PianoKey::from_midi_number),
    /// );
    ///
    /// // Since 13edt has an irrational step size (measured in semitones) every detuning is unique.
    /// assert_eq!(tunings.len(), 128);
    /// ```
    pub fn apply_channel_based_tuning(
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> (Self, Vec<Ratio>) {
        Self::apply_tuning_internal(
            tuning,
            keys,
            |_| (),
            |tuning_map| *tuning_map.get(&()).unwrap(),
        )
    }

    fn apply_tuning_internal<T, N: Copy + Hash + Eq>(
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
        mut extract_note: impl FnMut(Note) -> N,
        mut create_tuning: impl FnMut(HashMap<N, Ratio>) -> T,
    ) -> (Self, Vec<T>) {
        let mut tuning_map = HashMap::new();
        let mut key_map = HashMap::new();

        let mut to_distribute: Vec<_> = keys
            .into_iter()
            .flat_map(|key| {
                tuning
                    .maybe_pitch_of(key)
                    .map(|pitch| (key, pitch.find_in_tuning(())))
            })
            .collect();

        to_distribute.sort_by(|a, b| {
            a.1.deviation
                .partial_cmp(&b.1.deviation)
                .expect("Comparison failed (deviation)")
        });

        let mut channel_tunings = Vec::new();
        while !to_distribute.is_empty() {
            let mut notes_retuned_on_current_channel = HashMap::new();
            to_distribute.retain(|&(key, approx)| {
                let note = extract_note(approx.approx_value);
                let note_slot_is_usable = notes_retuned_on_current_channel
                    .get(&note)
                    .filter(|&&existing_deviation| {
                        !approx
                            .deviation
                            .deviation_from(existing_deviation)
                            .is_negligible()
                    })
                    .is_none();
                if note_slot_is_usable {
                    tuning_map.insert(note, approx.deviation);
                    key_map.insert(key, (channel_tunings.len(), approx.approx_value));
                    notes_retuned_on_current_channel.insert(note, approx.deviation);
                }
                !note_slot_is_usable
            });
            channel_tunings.push(create_tuning(tuning_map.clone()));
        }

        (
            Self {
                key_map,
                num_channels: channel_tunings.len(),
            },
            channel_tunings,
        )
    }

    /// Returns the channel and [`Note`] to be played when hitting a `key`.
    ///
    /// See [`ChannelTuner::apply_full_keyboard_tuning`] for an explanation of how to use this method.
    pub fn get_channel_and_note_for_key(&self, key: K) -> Option<(usize, Note)> {
        self.key_map.get(&key).copied()
    }

    /// Returns the number of channels that this [`ChannelTuner`] will make use of.
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }
}

/// Defines the amount by which any note of a keyboard is supposed to be detuned.
#[derive(Clone, Debug)]
pub struct FullKeyboardDetuning {
    tuning_map: HashMap<Note, Ratio>,
}

impl FullKeyboardDetuning {
    /// Returns an array with the pitches of all 128 MIDI notes.
    ///
    /// The pitches are measured in cents above MIDI number 0 (C-1, 8.18Hz).
    pub fn to_fluid_format(&self) -> [f64; 128] {
        let mut result = [0.0; 128];
        for (entry, midi_number) in result.iter_mut().zip(0..) {
            let detuning = self
                .tuning_map
                .get(&Note::from_midi_number(midi_number))
                .copied()
                .unwrap_or_default();
            *entry = Ratio::from_semitones(midi_number)
                .stretched_by(detuning)
                .as_cents()
        }
        result
    }

    pub fn to_mts_format(
        &self,
        device_id: DeviceId,
        tuning_program: u8,
    ) -> Result<SingleNoteTuningChangeMessage, SingleNoteTuningChangeError> {
        let tuning_changes = self
            .tuning_map
            .iter()
            .filter(|(note, _)| note.checked_midi_number().is_some())
            .map(|(note, &ratio)| {
                SingleNoteTuningChange::new(note.as_piano_key(), note.pitch() * ratio)
            });
        SingleNoteTuningChangeMessage::from_tuning_changes(
            tuning_changes,
            device_id,
            tuning_program,
        )
    }
}

/// Defines the amount by which any of the 12 notes of an octave is supposed to be detuned.
#[derive(Clone, Debug)]
pub struct OctaveBasedDetuning {
    tuning_map: HashMap<NoteLetter, Ratio>,
}

impl OctaveBasedDetuning {
    /// Returns an array with the deviations of all 12 note letters within an octave.
    ///
    /// The deviation is measured in cents above the 12-tone equal-tempered pitch.
    pub fn to_fluid_format(&self) -> [f64; 12] {
        let mut result = [0.0; 12];
        for (entry, midi_number) in result.iter_mut().zip(0..) {
            let note_letter = Note::from_midi_number(midi_number).letter_and_octave().0;
            *entry = self
                .tuning_map
                .get(&note_letter)
                .copied()
                .unwrap_or_default()
                .as_cents()
        }
        result
    }

    pub fn to_mts_format(
        &self,
        device_id: DeviceId,
        channels: impl Into<Channels>,
    ) -> Result<ScaleOctaveTuningMessage, ScaleOctaveTuningError> {
        let mut octave_tuning = ScaleOctaveTuning::default();
        for (&note_letter, &detuning) in &self.tuning_map {
            *octave_tuning.as_mut(note_letter) = detuning;
        }
        ScaleOctaveTuningMessage::from_scale_octave_tuning(&octave_tuning, channels, device_id)
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::{
        key::PianoKey,
        scala::{Kbm, KbmRoot, Scl},
    };

    use super::*;

    #[test]
    fn apply_full_keyboard_tuning() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(16))
            .build()
            .unwrap();

        let kbm = KbmRoot::from(Note::from_midi_number(62)).to_kbm();

        let (tuner, tunings) = ChannelTuner::apply_full_keyboard_tuning(
            (scl, kbm),
            (0..128).map(PianoKey::from_midi_number),
        );

        let (channels, notes) = extract_channels_and_notes(&tuner);
        assert_eq!(
            channels,
            [
                0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0
            ]
        );
        assert_eq!(
            notes,
            [
                15, 16, 17, 18, 18, 19, 20, 21, 21, 22, 23, 24, 24, 25, 26, 27, 27, 28, 29, 30, 30,
                31, 32, 33, 33, 34, 35, 36, 36, 37, 38, 39, 39, 40, 41, 42, 42, 43, 44, 45, 45, 46,
                47, 48, 48, 49, 50, 51, 51, 52, 53, 54, 54, 55, 56, 57, 57, 58, 59, 60, 60, 61, 62,
                63, 63, 64, 65, 66, 66, 67, 68, 69, 69, 70, 71, 72, 72, 73, 74, 75, 75, 76, 77, 78,
                78, 79, 80, 81, 81, 82, 83, 84, 84, 85, 86, 87, 87, 88, 89, 90, 90, 91, 92, 93, 93,
                94, 95, 96, 96, 97, 98, 99, 99, 100, 101, 102, 102, 103, 104, 105, 105, 106, 107,
                108, 108, 109, 110, 111
            ]
        );

        assert_eq!(tunings.len(), 2);
        assert_array_approx_eq(
            &tunings[0].to_fluid_format(),
            &[
                0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0,
                1200.0, 1300.0, 1400.0, 1550.0, 1625.0, 1700.0, 1775.0, 1925.0, 2000.0, 2075.0,
                2225.0, 2300.0, 2375.0, 2525.0, 2600.0, 2675.0, 2825.0, 2900.0, 2975.0, 3125.0,
                3200.0, 3275.0, 3425.0, 3500.0, 3575.0, 3725.0, 3800.0, 3875.0, 4025.0, 4100.0,
                4175.0, 4325.0, 4400.0, 4475.0, 4625.0, 4700.0, 4775.0, 4925.0, 5000.0, 5075.0,
                5225.0, 5300.0, 5375.0, 5525.0, 5600.0, 5675.0, 5825.0, 5900.0, 5975.0, 6125.0,
                6200.0, 6275.0, 6425.0, 6500.0, 6575.0, 6725.0, 6800.0, 6875.0, 7025.0, 7100.0,
                7175.0, 7325.0, 7400.0, 7475.0, 7625.0, 7700.0, 7775.0, 7925.0, 8000.0, 8075.0,
                8225.0, 8300.0, 8375.0, 8525.0, 8600.0, 8675.0, 8825.0, 8900.0, 8975.0, 9125.0,
                9200.0, 9275.0, 9425.0, 9500.0, 9575.0, 9725.0, 9800.0, 9875.0, 10025.0, 10100.0,
                10175.0, 10325.0, 10400.0, 10475.0, 10625.0, 10700.0, 10775.0, 10925.0, 11000.0,
                11075.0, 11200.0, 11300.0, 11400.0, 11500.0, 11600.0, 11700.0, 11800.0, 11900.0,
                12000.0, 12100.0, 12200.0, 12300.0, 12400.0, 12500.0, 12600.0, 12700.0,
            ],
        );
        assert_array_approx_eq(
            &tunings[1].to_fluid_format(),
            &[
                0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0,
                1200.0, 1300.0, 1400.0, 1550.0, 1625.0, 1700.0, 1850.0, 1925.0, 2000.0, 2150.0,
                2225.0, 2300.0, 2450.0, 2525.0, 2600.0, 2750.0, 2825.0, 2900.0, 3050.0, 3125.0,
                3200.0, 3350.0, 3425.0, 3500.0, 3650.0, 3725.0, 3800.0, 3950.0, 4025.0, 4100.0,
                4250.0, 4325.0, 4400.0, 4550.0, 4625.0, 4700.0, 4850.0, 4925.0, 5000.0, 5150.0,
                5225.0, 5300.0, 5450.0, 5525.0, 5600.0, 5750.0, 5825.0, 5900.0, 6050.0, 6125.0,
                6200.0, 6350.0, 6425.0, 6500.0, 6650.0, 6725.0, 6800.0, 6950.0, 7025.0, 7100.0,
                7250.0, 7325.0, 7400.0, 7550.0, 7625.0, 7700.0, 7850.0, 7925.0, 8000.0, 8150.0,
                8225.0, 8300.0, 8450.0, 8525.0, 8600.0, 8750.0, 8825.0, 8900.0, 9050.0, 9125.0,
                9200.0, 9350.0, 9425.0, 9500.0, 9650.0, 9725.0, 9800.0, 9950.0, 10025.0, 10100.0,
                10250.0, 10325.0, 10400.0, 10550.0, 10625.0, 10700.0, 10850.0, 10925.0, 11000.0,
                11075.0, 11200.0, 11300.0, 11400.0, 11500.0, 11600.0, 11700.0, 11800.0, 11900.0,
                12000.0, 12100.0, 12200.0, 12300.0, 12400.0, 12500.0, 12600.0, 12700.0,
            ],
        );
    }

    #[test]
    fn apply_full_keyboard_tuning_with_non_octave_scale() {
        let scl = Scl::builder()
            .push_ratio(Ratio::from_float(3.0).divided_into_equal_steps(13))
            .build()
            .unwrap();

        let kbm = KbmRoot::from(Note::from_midi_number(62)).to_kbm();

        let (tuner, tunings) = ChannelTuner::apply_full_keyboard_tuning(
            (scl, kbm),
            (0..128).map(PianoKey::from_midi_number),
        );

        let (channels, notes) = extract_channels_and_notes(&tuner);
        assert_eq!(
            channels,
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            notes,
            [
                -29, -27, -26, -24, -23, -21, -20, -18, -17, -16, -14, -13, -11, -10, -8, -7, -5,
                -4, -2, -1, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22, 24, 25, 27,
                28, 30, 31, 33, 34, 36, 37, 39, 40, 42, 43, 44, 46, 47, 49, 50, 52, 53, 55, 56, 58,
                59, 61, 62, 63, 65, 66, 68, 69, 71, 72, 74, 75, 77, 78, 80, 81, 82, 84, 85, 87, 88,
                90, 91, 93, 94, 96, 97, 99, 100, 102, 103, 104, 106, 107, 109, 110, 112, 113, 115,
                116, 118, 119, 121, 122, 123, 125, 126, 128, 129, 131, 132, 134, 135, 137, 138,
                140, 141, 142, 144, 145, 147, 148, 150, 151, 153, 154, 156, 157
            ]
        );

        assert_eq!(tunings.len(), 1);
    }

    #[test]
    fn apply_full_keyboard_tuning_reuse_pitches() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(15))
            .build()
            .unwrap();

        let kbm = Kbm::builder(Note::from_midi_number(60))
            .push_mapped_key(0)
            .push_mapped_key(3)
            .push_unmapped_key()
            .push_mapped_key(3) // Should be reused => No extra channel needed
            .formal_octave(5)
            .build()
            .unwrap();

        let (tuner, tunings) = ChannelTuner::apply_full_keyboard_tuning(
            (scl, kbm),
            (0..128).map(PianoKey::from_midi_number),
        );

        let (channels, notes) = extract_channels_and_notes(&tuner);
        // 999 means unmapped
        assert_eq!(
            channels,
            [
                0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0,
                0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0,
                0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0,
                0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0,
                0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0, 0, 0, 999, 0,
                0, 0, 999, 0, 0, 0, 999, 0
            ],
        );
        assert_eq!(
            notes,
            [
                0, 2, 999, 2, 4, 6, 999, 6, 8, 10, 999, 10, 12, 14, 999, 14, 16, 18, 999, 18, 20,
                22, 999, 22, 24, 26, 999, 26, 28, 30, 999, 30, 32, 34, 999, 34, 36, 38, 999, 38,
                40, 42, 999, 42, 44, 46, 999, 46, 48, 50, 999, 50, 52, 54, 999, 54, 56, 58, 999,
                58, 60, 62, 999, 62, 64, 66, 999, 66, 68, 70, 999, 70, 72, 74, 999, 74, 76, 78,
                999, 78, 80, 82, 999, 82, 84, 86, 999, 86, 88, 90, 999, 90, 92, 94, 999, 94, 96,
                98, 999, 98, 100, 102, 999, 102, 104, 106, 999, 106, 108, 110, 999, 110, 112, 114,
                999, 114, 116, 118, 999, 118, 120, 122, 999, 122, 124, 126, 999, 126
            ]
        );

        assert_eq!(tunings.len(), 1);
    }

    #[test]
    fn apply_octave_based_tuning() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(16))
            .build()
            .unwrap();

        let kbm = KbmRoot::from(Note::from_midi_number(62)).to_kbm();

        let (tuner, tunings) = ChannelTuner::apply_octave_based_tuning(
            (scl, kbm),
            (0..128).map(PianoKey::from_midi_number),
        );

        let (channels, notes) = extract_channels_and_notes(&tuner);
        assert_eq!(
            channels,
            [
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
                1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0
            ]
        );
        assert_eq!(
            notes,
            [
                15, 16, 17, 18, 18, 19, 20, 21, 21, 22, 23, 24, 24, 25, 26, 27, 27, 28, 29, 30, 30,
                31, 32, 33, 33, 34, 35, 36, 36, 37, 38, 39, 39, 40, 41, 42, 42, 43, 44, 45, 45, 46,
                47, 48, 48, 49, 50, 51, 51, 52, 53, 54, 54, 55, 56, 57, 57, 58, 59, 60, 60, 61, 62,
                63, 63, 64, 65, 66, 66, 67, 68, 69, 69, 70, 71, 72, 72, 73, 74, 75, 75, 76, 77, 78,
                78, 79, 80, 81, 81, 82, 83, 84, 84, 85, 86, 87, 87, 88, 89, 90, 90, 91, 92, 93, 93,
                94, 95, 96, 96, 97, 98, 99, 99, 100, 101, 102, 102, 103, 104, 105, 105, 106, 107,
                108, 108, 109, 110, 111
            ]
        );

        assert_eq!(tunings.len(), 2);
        assert_array_approx_eq(
            &tunings[0].to_fluid_format(),
            &[
                -25.0, 25.0, 0.0, -25.0, 25.0, 0.0, -25.0, 25.0, 0.0, -25.0, 25.0, 0.0,
            ],
        );
        assert_array_approx_eq(
            &tunings[1].to_fluid_format(),
            &[
                50.0, 25.0, 0.0, 50.0, 25.0, 0.0, 50.0, 25.0, 0.0, 50.0, 25.0, 0.0,
            ],
        );
    }

    #[test]
    fn apply_octave_based_tuning_with_non_octave_scale() {
        let scl = Scl::builder()
            .push_ratio(Ratio::from_float(3.0).divided_into_equal_steps(13))
            .build()
            .unwrap();

        let kbm = KbmRoot::from(Note::from_midi_number(62)).to_kbm();

        let (tuner, tunings) = ChannelTuner::apply_octave_based_tuning(
            (scl, kbm),
            (0..128).map(PianoKey::from_midi_number),
        );

        let (channels, notes) = extract_channels_and_notes(&tuner);
        assert_eq!(
            channels,
            [
                9, 3, 9, 3, 9, 2, 5, 2, 5, 11, 5, 8, 5, 8, 2, 8, 2, 8, 2, 5, 2, 5, 11, 5, 11, 5, 9,
                2, 9, 2, 8, 2, 8, 2, 6, 12, 6, 12, 5, 8, 5, 8, 2, 8, 2, 8, 1, 4, 1, 4, 10, 4, 7, 4,
                7, 1, 7, 1, 7, 1, 4, 1, 4, 10, 4, 10, 4, 8, 1, 8, 1, 7, 1, 7, 1, 5, 11, 5, 11, 4,
                7, 4, 7, 1, 7, 1, 7, 0, 3, 0, 3, 9, 3, 6, 3, 6, 0, 6, 0, 6, 0, 3, 0, 3, 9, 3, 9, 3,
                7, 0, 7, 0, 6, 0, 6, 0, 4, 10, 4, 10, 3, 6, 3, 6, 0, 6, 0, 6
            ]
        );
        assert_eq!(
            notes,
            [
                -29, -27, -26, -24, -23, -21, -20, -18, -17, -16, -14, -13, -11, -10, -8, -7, -5,
                -4, -2, -1, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22, 24, 25, 27,
                28, 30, 31, 33, 34, 36, 37, 39, 40, 42, 43, 44, 46, 47, 49, 50, 52, 53, 55, 56, 58,
                59, 61, 62, 63, 65, 66, 68, 69, 71, 72, 74, 75, 77, 78, 80, 81, 82, 84, 85, 87, 88,
                90, 91, 93, 94, 96, 97, 99, 100, 102, 103, 104, 106, 107, 109, 110, 112, 113, 115,
                116, 118, 119, 121, 122, 123, 125, 126, 128, 129, 131, 132, 134, 135, 137, 138,
                140, 141, 142, 144, 145, 147, 148, 150, 151, 153, 154, 156, 157
            ]
        );

        assert_eq!(tunings.len(), 13); // The number of channels is high since no note letter can be reused
    }

    fn extract_channels_and_notes(tuner: &ChannelTuner<PianoKey>) -> (Vec<usize>, Vec<i32>) {
        (0..128)
            .map(|midi_number| {
                let channel_and_note = tuner
                    .get_channel_and_note_for_key(PianoKey::from_midi_number(midi_number))
                    .unwrap_or((999, Note::from_midi_number(999)));
                (channel_and_note.0, channel_and_note.1.midi_number())
            })
            .unzip()
    }

    fn assert_array_approx_eq(left: &[f64], right: &[f64]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert_approx_eq!(left, right)
        }
    }
}
