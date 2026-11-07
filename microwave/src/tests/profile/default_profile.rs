use assert_approx_eq::assert_approx_eq;

use crate::control::LiveParameter;
use crate::profile::MicrowaveProfile;
use crate::profile::get_default_profile;
use crate::tests::profile::fixture::ProfileFixture;

#[test]
fn default_profile_contains_live_and_skip_back_recorders() {
    let yaml = serde_yaml::to_string(&get_default_profile()).unwrap();
    let profile: MicrowaveProfile = serde_yaml::from_str(&yaml).unwrap();
    let fixture = ProfileFixture::from_profile(profile);

    let prefixes = fixture
        .recorders()
        .map(|recorder| recorder.file_prefix.as_str())
        .collect::<Vec<_>>();
    assert_eq!(prefixes, ["microwave", "microwave_past"]);

    let live = fixture.recorder("microwave");
    assert_approx_eq!(live.skip_back_secs, 0.0);
    assert_eq!(
        ProfileFixture::recording_parameter(live),
        LiveParameter::Sound1
    );

    let skip_back = fixture.recorder("microwave_past");
    assert_approx_eq!(skip_back.skip_back_secs, 60.0);
    assert_eq!(
        ProfileFixture::recording_parameter(skip_back),
        LiveParameter::Sound2
    );
}
