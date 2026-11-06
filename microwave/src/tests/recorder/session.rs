use crate::tests::recorder::fixture::BUFFER_SIZE;
use crate::tests::recorder::fixture::RecorderFixture;
use crate::tests::recorder::fixture::SAMPLE_RATE;

#[test]
fn recorder_writes_stereo_wav() {
    let mut fixture = RecorderFixture::new();

    fixture.set_active(true);
    fixture.render(256, 0.25, 0.75);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);

    let samples = RecorderFixture::read_wav(&files[0]);
    assert_eq!(samples.len(), 2 * 256);
    for frame in samples.as_chunks::<2>().0 {
        assert_eq!(frame, &[0.25, 0.75]);
    }
}

#[test]
fn recorder_rapid_restart_creates_new_file() {
    let mut fixture = RecorderFixture::new();

    fixture.set_active(true);
    fixture.render(64, 0.25, 0.75);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);

    fixture.set_active(true);
    fixture.render(64, 0.5, 0.5);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 2);

    let first = RecorderFixture::read_wav(&files[0]);
    assert_eq!(first.len(), 2 * 64);
    for frame in first.as_chunks::<2>().0 {
        assert_eq!(frame, &[0.25, 0.75]);
    }

    let second = RecorderFixture::read_wav(&files[1]);
    assert_eq!(second.len(), 2 * 64);
    for frame in second.as_chunks::<2>().0 {
        assert_eq!(frame, &[0.5, 0.5]);
    }
}

#[test]
fn recorder_stops_after_overflow() {
    let mut fixture = RecorderFixture::new();
    fixture.set_active(true);

    let max_frames = 10 * SAMPLE_RATE as usize;
    for _ in 0..max_frames / BUFFER_SIZE + 1 {
        fixture.render(BUFFER_SIZE, 0.25, 0.75);
    }

    let mut reported = false;
    for _ in 0..1000 {
        fixture.poll();

        let events = fixture.drain_events();
        if events.iter().any(|event| {
            matches!(
                event,
                crate::pipeline::PipelineEvent::WavRecorder(event)
                    if event.error.as_deref().is_some_and(|error| error.contains("overflow"))
            )
        }) {
            reported = true;
            break;
        }
    }

    assert!(reported);
}
