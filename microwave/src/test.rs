use std::fs;

use serde_yaml;

use crate::profile::MicrowaveProfile;

#[test]
fn deserialize_profiles() {
    let yml_files = fs::read_dir(".").unwrap().filter_map(|entry| {
        let path = entry.unwrap().path();
        (path.is_file() && path.extension().unwrap_or_default() == "yml").then_some(path)
    });

    for file_path in yml_files {
        let contents = fs::read_to_string(&file_path).unwrap();
        serde_yaml::from_str::<MicrowaveProfile>(&contents).unwrap();
    }
}
