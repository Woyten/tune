use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
};
use tune::{key::PianoKey, note::Note, pitch::Pitched, ratio::Ratio, tuning::Tuning};

pub struct ChannelTuner {
    key_map: HashMap<PianoKey, (u8, Note)>,
    lowest_key: PianoKey,
    highest_key: PianoKey,
}

impl ChannelTuner {
    pub fn new() -> Self {
        Self {
            key_map: HashMap::new(),
            lowest_key: PianoKey::from_midi_number(0),
            highest_key: PianoKey::from_midi_number(0),
        }
    }

    pub fn set_tuning(&mut self, tuning: &impl Tuning<PianoKey>) -> Option<Vec<[f64; 128]>> {
        self.lowest_key = tuning
            .find_by_pitch(Note::from_midi_number(0).pitch())
            .approx_value
            .plus_steps(1);
        self.highest_key = tuning
            .find_by_pitch(Note::from_midi_number(127).pitch())
            .approx_value;

        let mut keys_to_distribute_over_channels = HashMap::new();

        for midi_number in self.lowest_key.midi_number()..self.highest_key.midi_number() {
            let key = PianoKey::from_midi_number(midi_number);
            let pitch = tuning.pitch_of(key);
            let var_name = pitch.find_in(&());
            let nearest_note = var_name.approx_value;
            keys_to_distribute_over_channels.insert(key, (nearest_note, pitch));
        }

        let mut channel_tunings = Vec::new();
        self.key_map.clear();

        for channel in 0..16 {
            if keys_to_distribute_over_channels.is_empty() {
                break;
            }

            let mut channel_tuning = [0.0; 128];
            let mut notes_retuned_on_current_channel = HashSet::new();

            keys_to_distribute_over_channels.retain(|&piano_key, &mut (nearest_note, pitch)| {
                let tuning_diff = Ratio::between_pitches(Note::from_midi_number(0), pitch);
                if notes_retuned_on_current_channel.contains(&nearest_note) {
                    true
                } else {
                    channel_tuning[usize::try_from(nearest_note.midi_number()).unwrap()] =
                        tuning_diff.as_cents();
                    notes_retuned_on_current_channel.insert(nearest_note);
                    self.key_map.insert(piano_key, (channel, nearest_note));
                    false
                }
            });

            channel_tunings.push(channel_tuning);
        }

        if keys_to_distribute_over_channels.is_empty() {
            Some(channel_tunings)
        } else {
            None
        }
    }

    /// Returns the channel and note that needs to be played when hitting a key.
    pub fn get_channel_and_note_for_key(&self, key: PianoKey) -> Option<(u8, Note)> {
        self.key_map.get(&key).copied()
    }

    pub fn boundaries(&self) -> (PianoKey, PianoKey) {
        (self.lowest_key, self.highest_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tune::scala::{Kbm, Scl};

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

            let mut tuner = ChannelTuner::new();
            tuner.set_tuning(&(scale, Kbm::root_at(Note::from_midi_number(62))));
        }
    }
}
