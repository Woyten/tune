//! Generate tuning maps to enhance the capabilities of synthesizers with limited tuning support.

use crate::{
    key::PianoKey, mts::ScaleOctaveTuning, note::Note, pitch::Pitched, ratio::Ratio, tuning::Tuning,
};
use std::collections::{HashMap, HashSet};

/// Maps [`PianoKey`]s accross multiple channels to overcome several tuning limitations.
pub struct ChannelTuner {
    key_map: HashMap<PianoKey, (usize, Note)>,
    num_channels: usize,
}

impl ChannelTuner {
    pub fn new() -> Self {
        Self {
            key_map: HashMap::new(),
            num_channels: 0,
        }
    }

    /// Distributes the provided [`Tuning`] accross multiple channels, s.t. each note is only detuned once per channel and by 50c at most.
    ///
    /// This works around a restriction of some synthesizers (e.g. fluidlite) where the pitch per note can be customized but the sound sample per note cannot. Apply this strategy if your samples sound as if they were played back in slow motion or time lapse.
    ///
    /// The key bounds are [left inclusive, right exclusive).
    pub fn apply_full_keyboard_tuning(
        &mut self,
        tuning: &impl Tuning<PianoKey>,
        lower_key_bound: PianoKey,
        upper_key_bound: PianoKey,
    ) -> Vec<ChannelTuning> {
        self.key_map.clear();

        // BTreeMap used to guarantee a stable distribution accross channels
        let mut keys_to_distribute_over_channels = Vec::new();
        for midi_number in lower_key_bound.midi_number()..upper_key_bound.midi_number() {
            let key = PianoKey::from_midi_number(midi_number);
            let pitch = tuning.pitch_of(key);
            let detune_for_numerical_stability = Ratio::from_cents(0.01);
            let nearest_note = (pitch * detune_for_numerical_stability)
                .find_in(&())
                .approx_value;
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

    /// Distributes the provided [`Tuning`] accross multiple channels, s.t. each note *letter* is only detuned once per channel and by 50c at most.
    ///
    /// This strategy can be applied on synthesizer having octave-based tuning support but no full keyboard tuning support.
    pub fn apply_octave_based_tuning(
        &mut self,
        tuning: &impl Tuning<PianoKey>,
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
            .apply_full_keyboard_tuning(tuning, lowest_key, highest_key)
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

    /// Returns the channel and [`Note`] to be played when hitting a [`PianoKey`].
    pub fn get_channel_and_note_for_key(&self, key: PianoKey) -> Option<(usize, Note)> {
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
    pub fn to_fluidlite_format(&self) -> [f64; 128] {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scala::{Kbm, Scl};

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
                &(scale, Kbm::root_at(Note::from_midi_number(62))),
                PianoKey::from_midi_number(0),
                PianoKey::from_midi_number(128),
            ) {
                channel_tuning.to_fluidlite_format();
            }
        }
    }
}
