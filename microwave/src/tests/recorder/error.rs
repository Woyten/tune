use crate::pipeline::PipelineEvent;
use crate::tests::recorder::fixture::RecorderFixture;

#[test]
fn recorder_create_failure_is_reported() {
    let mut fixture = RecorderFixture::new_with_failing_prefix();

    fixture.set_active(true);
    fixture.render(64, 0.25, 0.75);
    fixture.poll();

    let events = fixture.drain_events();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            PipelineEvent::WavRecorder(event)
                if event.file_name.is_none()
                    && event.error.as_deref().is_some_and(|error| error.contains("create"))
        )
    }));

    assert!(fixture.files().is_empty());
}
