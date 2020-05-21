use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};

pub struct KeypressTracker<F, K> {
    finger_position: HashMap<F, K>,
    num_fingers_on_key: HashMap<K, usize>,
}

impl<F: Eq + Hash, K: Eq + Hash + Copy> KeypressTracker<F, K> {
    pub fn new() -> Self {
        Self {
            finger_position: HashMap::new(),
            num_fingers_on_key: HashMap::new(),
        }
    }

    pub fn place_finger_at(&mut self, finger: F, new_key: K) -> Result<PlaceAction, IllegalState> {
        match self.finger_position.entry(finger) {
            Entry::Occupied(_) => Err(IllegalState),
            Entry::Vacant(vacant) => {
                vacant.insert(new_key);
                Ok(increase_key_count(&mut self.num_fingers_on_key, new_key))
            }
        }
    }

    pub fn move_finger_to(
        &mut self,
        finger: &F,
        new_key: K,
    ) -> Result<(LiftAction<K>, PlaceAction), IllegalState> {
        let old_key = match self.finger_position.get_mut(finger) {
            Some(old_key) => old_key,
            None => return Err(IllegalState),
        };

        if new_key == *old_key {
            return Ok((
                LiftAction::KeyRemainsPressed,
                PlaceAction::KeyAlreadyPressed,
            ));
        }

        let lift_update = decrease_key_count(&mut self.num_fingers_on_key, *old_key);
        let place_update = increase_key_count(&mut self.num_fingers_on_key, new_key);

        *old_key = new_key;

        Ok((lift_update, place_update))
    }

    pub fn lift_finger(&mut self, finger: &F) -> Result<LiftAction<K>, IllegalState> {
        match self.finger_position.remove(finger) {
            Some(old_key) => Ok(decrease_key_count(&mut self.num_fingers_on_key, old_key)),
            None => Err(IllegalState),
        }
    }
}

fn increase_key_count<K: Eq + Hash>(
    num_fingers_on_key: &mut HashMap<K, usize>,
    key: K,
) -> PlaceAction {
    let num_fingers = num_fingers_on_key.entry(key).or_insert(0);

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
            Err(IllegalState)
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
            .place_finger_at("additionial finger A", "occupied location")
            .unwrap();
        keypress_tracker
            .place_finger_at("additionial finger B", "another occupied location")
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
