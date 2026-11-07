use crate::control::LiveParameter;
use crate::magnetron::source::LfSource;
use crate::magnetron::source::LfSourceExpr;
use crate::pipeline::PipelineStageSpec;
use crate::profile::MicrowaveProfile;
use crate::profile::PipelineParam;
use crate::recorder::WavRecorderSpec;

pub struct ProfileFixture {
    profile: MicrowaveProfile,
}

impl ProfileFixture {
    pub fn from_profile(profile: MicrowaveProfile) -> Self {
        Self { profile }
    }

    pub fn recorder(&self, file_prefix: &str) -> &WavRecorderSpec<PipelineParam> {
        self.recorders()
            .find(|recorder| recorder.file_prefix == file_prefix)
            .unwrap_or_else(|| panic!("No recorder with file prefix `{file_prefix}`"))
    }

    pub fn recorders(&self) -> impl Iterator<Item = &WavRecorderSpec<PipelineParam>> {
        self.profile.stages.iter().filter_map(|stage| match stage {
            PipelineStageSpec::WavRecorder(spec) => Some(spec),
            _ => None,
        })
    }

    pub fn recording_parameter(spec: &WavRecorderSpec<PipelineParam>) -> LiveParameter {
        match &spec.recording_active {
            LfSource::Expr(expr) => match expr.as_ref() {
                LfSourceExpr::Controller { kind, .. } => *kind,
                other => panic!("Unexpected recording_active expression: {other:?}"),
            },
            other => panic!("Unexpected recording_active source: {other:?}"),
        }
    }
}
