use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
};
use tune::{key::PianoKey, note::Note, pitch::Pitched, ratio::Ratio, tuning::Tuning};

pub struct ChannelTuner {
    key_map: HashMap<PianoKey, (u8, Note)>,
}

impl ChannelTuner {
    pub fn new() -> Self {
        Self {
            key_map: HashMap::new(),
        }
    }

    pub fn set_tuning(&mut self, tuning: &impl Tuning<PianoKey>) -> Option<Vec<[f64; 128]>> {
        let monotony_hint = i32::try_from(tuning.monotony()).expect("Monotony hint too large");
        let lowest_key = tuning
            .find_by_pitch(Note::from_midi_number(0).pitch())
            .approx_value
            .plus_steps(-monotony_hint);
        let highest_key = tuning
            .find_by_pitch(Note::from_midi_number(128).pitch())
            .approx_value
            .plus_steps(monotony_hint);

        let mut keys_to_distribute_over_channels = HashMap::new();

        for midi_number in lowest_key.midi_number()..highest_key.midi_number() {
            let key = PianoKey::from_midi_number(midi_number);
            let pitch = tuning.pitch_of(key);
            let nearest_note = pitch.find_in(&()).approx_value;
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
                    if let Some(x) = channel_tuning.get_mut(nearest_note.midi_number() as usize) {
                        *x = tuning_diff.as_cents();
                        notes_retuned_on_current_channel.insert(nearest_note);
                    }
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
