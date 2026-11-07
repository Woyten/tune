use async_std::task;

use crate::profile::MicrowaveProfile;

#[test]
fn deserialize_provided_profiles() {
    for file_name in ["audio-effect.yml", "sympathetic.yml"] {
        task::block_on(MicrowaveProfile::load(file_name))
            .map_err(|err| err.to_string())
            .unwrap();
    }
}
