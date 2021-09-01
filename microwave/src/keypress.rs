use std::{collections::HashMap, hash::Hash};

pub struct KeypressTracker<F, L> {
    finger_position: HashMap<F, L>,
    num_fingers_on_key: HashMap<L, usize>,
}

impl<F, L> KeypressTracker<F, L> {
    pub fn new() -> Self {
        Self {
            finger_position: HashMap::new(),
            num_fingers_on_key: HashMap::new(),
        }
    }
}

impl<F: Eq + Hash, L: Eq + Hash + Copy> KeypressTracker<F, L> {
    #[allow(clippy::map_entry)]
    pub fn place_finger_at(&mut self, finger: F, new_location: L) -> Result<PlaceAction, F> {
        if self.finger_position.contains_key(&finger) {
            Err(finger)
        } else {
            self.finger_position.insert(finger, new_location);
            Ok(increase_key_count(
                &mut self.num_fingers_on_key,
                new_location,
            ))
        }
    }

    pub fn move_finger_to(
        &mut self,
        finger: &F,
        new_location: L,
    ) -> Result<(LiftAction<L>, PlaceAction), IllegalState> {
        let old_location = match self.finger_position.get_mut(finger) {
            Some(old_key) => old_key,
            None => return Err(IllegalState),
        };

        if new_location == *old_location {
            return Ok((
                LiftAction::KeyRemainsPressed,
                PlaceAction::KeyAlreadyPressed,
            ));
        }

        let lift_update = decrease_key_count(&mut self.num_fingers_on_key, *old_location);
        let place_update = increase_key_count(&mut self.num_fingers_on_key, new_location);

        *old_location = new_location;

        Ok((lift_update, place_update))
    }

    pub fn lift_finger(&mut self, finger: &F) -> Result<LiftAction<L>, IllegalState> {
        match self.finger_position.remove(finger) {
            Some(old_location) => Ok(decrease_key_count(
                &mut self.num_fingers_on_key,
                old_location,
            )),
            None => Err(IllegalState),
        }
    }

    pub fn location_of(&self, finger: &F) -> Option<&L> {
        self.finger_position.get(finger)
    }
}

fn increase_key_count<L: Eq + Hash>(
    num_fingers_on_key: &mut HashMap<L, usize>,
    location: L,
) -> PlaceAction {
    let num_fingers = num_fingers_on_key.entry(location).or_insert(0);

    *num_fingers += 1;

    if *num_fingers > 1 {
        PlaceAction::KeyAlreadyPressed
    } else {
        PlaceAction::KeyPressed
    }
}

fn decrease_key_count<K: Eq + Hash>(
    num_fingers_on_key: &mut HashMap<K, usize>,
    key: K,
) -> LiftAction<K> {
    let num_fingers = num_fingers_on_key.get_mut(&key).expect("Key not found");

    if *num_fingers > 1 {
        *num_fingers -= 1;
        LiftAction::KeyRemainsPressed
    } else {
        num_fingers_on_key.remove(&key);
        LiftAction::KeyReleased(key)
    }
}

#[derive(Debug)]
pub struct IllegalState;

pub enum PlaceAction {
    KeyPressed,
    KeyAlreadyPressed,
}

pub enum LiftAction<K> {
    KeyReleased(K),
    KeyRemainsPressed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_and_lift_finger() {
        let mut keypress_tracker = KeypressTracker::new();
        keypress_tracker
            .place_finger_at("already placed finger", "occupied location")
            .unwrap();

        assert!(matches!(
            keypress_tracker.place_finger_at("finger A", "empty location"),
            Ok(PlaceAction::KeyPressed)
        ));
        assert!(matches!(
            keypress_tracker.lift_finger(&"finger A"),
            Ok(LiftAction::KeyReleased("empty location"))
        ));
        assert!(matches!(
            keypress_tracker.place_finger_at("finger B", "occupied location"),
            Ok(PlaceAction::KeyAlreadyPressed)
        ));
        assert!(matches!(
            keypress_tracker.lift_finger(&"finger B"),
            Ok(LiftAction::KeyRemainsPressed)
        ));
        assert!(matches!(
            keypress_tracker.lift_finger(&"already placed finger"),
            Ok(LiftAction::KeyReleased("occupied location"))
        ));
    }

    #[test]
    fn place_and_lift_finger_illegally() {
        let mut keypress_tracker = KeypressTracker::new();
        keypress_tracker
            .place_finger_at("already placed finger", "any location")
            .unwrap();

        assert!(matches!(
            keypress_tracker.place_finger_at("already placed finger", "any location"),
            Err("already placed finger")
        ));
        assert!(matches!(
            keypress_tracker.lift_finger(&"already placed finger"),
            Ok(LiftAction::KeyReleased("any location"))
        ));
        assert!(matches!(
            keypress_tracker.lift_finger(&"already placed finger"),
            Err(IllegalState)
        ));
    }

    #[test]
    fn move_finger() {
        let mut keypress_tracker = KeypressTracker::new();
        keypress_tracker
            .place_finger_at("finger", "initial location")
            .unwrap();
        keypress_tracker
            .place_finger_at("additional finger A", "occupied location")
            .unwrap();
        keypress_tracker
            .place_finger_at("additional finger B", "another occupied location")
            .unwrap();

        assert!(matches!(
            keypress_tracker.move_finger_to(&"finger", "initial location"),
            Ok((
                LiftAction::KeyRemainsPressed,
                PlaceAction::KeyAlreadyPressed
            ))
        ));
        assert!(matches!(
            keypress_tracker.move_finger_to(&"finger", "new location"),
            Ok((
                LiftAction::KeyReleased("initial location"),
                PlaceAction::KeyPressed
            ))
        ));
        assert!(matches!(
            keypress_tracker.move_finger_to(&"finger", "occupied location"),
            Ok((
                LiftAction::KeyReleased("new location"),
                PlaceAction::KeyAlreadyPressed
            ))
        ));
        assert!(matches!(
            keypress_tracker.move_finger_to(&"finger", "another occupied location"),
            Ok((
                LiftAction::KeyRemainsPressed,
                PlaceAction::KeyAlreadyPressed
            ))
        ));
        assert!(matches!(
            keypress_tracker.move_finger_to(&"finger", "initial location"),
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed))
        ));
    }

    #[test]
    fn move_or_lift_unknown_fingers() {
        let mut keypress_tracker = KeypressTracker::new();

        assert!(matches!(
            keypress_tracker.move_finger_to(&"unknown finger", "any location"),
            Err(IllegalState)
        ));

        assert!(matches!(
            keypress_tracker.lift_finger(&"unknown finger"),
            Err(IllegalState)
        ));
    }
}
