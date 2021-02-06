//! Generate tuning maps to enhance the capabilities of synthesizers with limited tuning support.

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use crate::{
    key::PianoKey,
    mts::{
        DeviceId, ScaleOctaveTuning, SingleNoteTuningChange, SingleNoteTuningChangeError,
        SingleNoteTuningChangeMessage,
    },
    note::Note,
    pitch::{Pitched, Ratio},
    tuning::Tuning,
};

/// Maps keys accross multiple channels to overcome several tuning limitations.
pub struct ChannelTuner<K> {
    key_map: HashMap<K, (usize, Note)>,
    num_channels: usize,
}

impl<K: Copy + Eq + Hash> ChannelTuner<K> {
    #[allow(clippy::new_without_default)] // This is only a temporary API
    pub fn new() -> Self {
        Self {
            key_map: HashMap::new(),
            num_channels: 0,
        }
    }

    /// Distributes the provided [`Tuning`] accross multiple channels, s.t. each note is only detuned once per channel and by 50c at most.
    ///
    /// This works around a restriction of some synthesizers (e.g. FluidSynth) where the pitch per note can be customized but the sound sample per note cannot. Apply this strategy if your samples sound as if they were played back in slow motion or time lapse.
    pub fn apply_full_keyboard_tuning(
        &mut self,
        tuning: impl Tuning<K>,
        scale_degrees: impl IntoIterator<Item = K>,
    ) -> Vec<ChannelTuning> {
        self.key_map.clear();

        // BTreeMap used to guarantee a stable distribution accross channels
        let mut keys_to_distribute_over_channels = Vec::new();
        for key in scale_degrees {
            let pitch = tuning.pitch_of(key);
            let nearest_note = pitch.find_in_tuning(()).approx_value;
            keys_to_distribute_over_channels.push((key, nearest_note, pitch));
        }

        let mut channel_tunings = Vec::new();
        let mut current_channel = 0;
        while !keys_to_distribute_over_channels.is_empty() {
            let mut tuning_map = HashMap::new();

            let mut notes_retuned_on_current_channel = HashSet::new();
            keys_to_distribute_over_channels = keys_to_distribute_over_channels
                .into_iter()
                .filter(|&(piano_key, nearest_note, pitch)| {
                    if notes_retuned_on_current_channel.contains(&nearest_note) {
                        true
                    } else {
                        tuning_map
                            .insert(nearest_note, Ratio::between_pitches(nearest_note, pitch));
                        notes_retuned_on_current_channel.insert(nearest_note);
                        self.key_map
                            .insert(piano_key, (current_channel, nearest_note));
                        false
                    }
                })
                .collect();

            channel_tunings.push(ChannelTuning { tuning_map });
            current_channel += 1;
        }

        self.num_channels = channel_tunings.len();
        channel_tunings
    }
}

impl ChannelTuner<PianoKey> {
    /// Distributes the provided [`Tuning`] accross multiple channels, s.t. each note *letter* is only detuned once per channel and by 50c at most.
    ///
    /// This strategy can be applied on synthesizer having octave-based tuning support but no full keyboard tuning support.
    pub fn apply_octave_based_tuning(
        &mut self,
        tuning: impl Tuning<PianoKey>,
        period: Ratio,
    ) -> Result<Vec<ScaleOctaveTuning>, OctaveBasedTuningError> {
        let num_periods_per_octave = Ratio::octave().num_equal_steps_of_size(period);
        if (num_periods_per_octave - num_periods_per_octave.round()).abs() > 1e-6 {
            return Err(OctaveBasedTuningError::NonOctaveTuning);
        };

        let padding = period;

        let lowest_key = tuning
            .find_by_pitch(Note::from_midi_number(0).pitch() / padding)
            .approx_value;

        let highest_key = tuning
            .find_by_pitch(Note::from_midi_number(128).pitch() * padding)
            .approx_value;

        let mut octave_tuning = ScaleOctaveTuning::default();
        Ok(self
            .apply_full_keyboard_tuning(
                tuning,
                (lowest_key.midi_number()..highest_key.midi_number())
                    .map(PianoKey::from_midi_number),
            )
            .into_iter()
            .map(|channel_tuning| {
                // Only use the first 12 notes for the octave tuning
                for midi_number in 0..12 {
                    let note = Note::from_midi_number(midi_number);
                    let letter = note.letter_and_octave().0;
                    if let Some(&detuning) = channel_tuning.tuning_map.get(&note) {
                        *octave_tuning.as_mut(letter) = detuning;
                    }
                }
                octave_tuning.clone()
            })
            .collect())
    }
}

impl<K: Copy + Eq + Hash> ChannelTuner<K> {
    /// Returns the channel and [`Note`] to be played when hitting a `key`.
    pub fn get_channel_and_note_for_key(&self, key: K) -> Option<(usize, Note)> {
        self.key_map.get(&key).copied()
    }

    /// Returns the number of channels that this tuning will use.
    pub fn num_channels(&self) -> usize {
        self.num_channels
    }
}

#[derive(Copy, Clone, Debug)]
pub enum OctaveBasedTuningError {
    NonOctaveTuning,
}

pub struct ChannelTuning {
    tuning_map: HashMap<Note, Ratio>,
}

impl ChannelTuning {
    /// Returns an array with the pitches of all 128 MIDI notes.
    ///
    /// The pitches are measured in cents above MIDI number 0 (C-1, 8.18Hz).
    pub fn to_fluid_format(&self) -> [f64; 128] {
        let mut result = [0.0; 128];
        for (note, &detuning) in &self.tuning_map {
            let midi_number = note.midi_number();
            if let Some(entry) = result.get_mut(midi_number as usize) {
                *entry = Ratio::from_semitones(midi_number)
                    .stretched_by(detuning)
                    .as_cents()
            }
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

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::scala::{KbmRoot, Scl};

    use super::*;

    #[test]
    fn apply_full_keyboard_tuning() {
        let scl = Scl::builder()
            .push_ratio(Ratio::octave().divided_into_equal_steps(16))
            .build()
            .unwrap();

        let kbm = KbmRoot::from(Note::from_midi_number(62));

        let mut tuner = ChannelTuner::new();
        let tunings =
            tuner.apply_full_keyboard_tuning((scl, kbm), (0..128).map(PianoKey::from_midi_number));

        assert_eq!(tunings.len(), 2);

        assert_array_approx_eq(
            &tunings[0].to_fluid_format(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1550.0,
                1625.0, 1700.0, 1775.0, 1925.0, 2000.0, 2075.0, 2225.0, 2300.0, 2375.0, 2525.0,
                2600.0, 2675.0, 2825.0, 2900.0, 2975.0, 3125.0, 3200.0, 3275.0, 3425.0, 3500.0,
                3575.0, 3725.0, 3800.0, 3875.0, 4025.0, 4100.0, 4175.0, 4325.0, 4400.0, 4475.0,
                4625.0, 4700.0, 4775.0, 4925.0, 5000.0, 5075.0, 5225.0, 5300.0, 5375.0, 5525.0,
                5600.0, 5675.0, 5825.0, 5900.0, 5975.0, 6125.0, 6200.0, 6275.0, 6425.0, 6500.0,
                6575.0, 6725.0, 6800.0, 6875.0, 7025.0, 7100.0, 7175.0, 7325.0, 7400.0, 7475.0,
                7625.0, 7700.0, 7775.0, 7925.0, 8000.0, 8075.0, 8225.0, 8300.0, 8375.0, 8525.0,
                8600.0, 8675.0, 8825.0, 8900.0, 8975.0, 9125.0, 9200.0, 9275.0, 9425.0, 9500.0,
                9575.0, 9725.0, 9800.0, 9875.0, 10025.0, 10100.0, 10175.0, 10325.0, 10400.0,
                10475.0, 10625.0, 10700.0, 10775.0, 10925.0, 11000.0, 11075.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 00.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
        );
        assert_array_approx_eq(
            &tunings[1].to_fluid_format(),
            &[
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1850.0, 0.0, 0.0, 2150.0, 0.0, 0.0, 2450.0, 0.0, 0.0, 2750.0, 0.0, 0.0,
                3050.0, 0.0, 0.0, 3350.0, 0.0, 0.0, 3650.0, 0.0, 0.0, 3950.0, 0.0, 0.0, 4250.0,
                0.0, 0.0, 4550.0, 0.0, 0.0, 4850.0, 0.0, 0.0, 5150.0, 0.0, 0.0, 5450.0, 0.0, 0.0,
                5750.0, 0.0, 0.0, 6050.0, 0.0, 0.0, 6350.0, 0.0, 0.0, 6650.0, 0.0, 0.0, 6950.0,
                0.0, 0.0, 7250.0, 0.0, 0.0, 7550.0, 0.0, 0.0, 7850.0, 0.0, 0.0, 8150.0, 0.0, 0.0,
                8450.0, 0.0, 0.0, 8750.0, 0.0, 0.0, 9050.0, 0.0, 0.0, 9350.0, 0.0, 0.0, 9650.0,
                0.0, 0.0, 9950.0, 0.0, 0.0, 10250.0, 0.0, 0.0, 10550.0, 0.0, 0.0, 10850.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0,
            ],
        );
    }

    fn assert_array_approx_eq(left: &[f64], right: &[f64]) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert_approx_eq!(left, right)
        }
    }

    #[test]
    fn set_tuning_must_not_crash() {
        for ratio in &[
            "7:24:2",   // Scale with out-of-range boundary notes: (-1.0 and 128.5)
            "1:1000:2", // A high density scale
        ] {
            let scale = Scl::builder()
                .push_ratio(ratio.parse().unwrap())
                .build()
                .unwrap();

            for channel_tuning in ChannelTuner::new().apply_full_keyboard_tuning(
                &(scale, KbmRoot::from(Note::from_midi_number(62))),
                (0..128).map(PianoKey::from_midi_number),
            ) {
                channel_tuning.to_fluid_format();
            }
        }
    }
}
