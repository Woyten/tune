use assert_approx_eq::assert_approx_eq;

use crate::tests::recorder::fixture;
use crate::tests::recorder::fixture::RecorderFixture;

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
fn recorder_includes_skip_back_samples() {
    let mut fixture = RecorderFixture::new_with_skip_back(3.0 / f64::from(fixture::SAMPLE_RATE));

    for sample in 1..=5 {
        let sample = f64::from(sample);
        fixture.render(1, sample, -sample);
    }

    fixture.set_active(true);
    for sample in 6..=7 {
        let sample = f64::from(sample);
        fixture.render(1, sample, -sample);
    }
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);

    let samples = RecorderFixture::read_wav(&files[0]);
    assert_samples(
        &samples,
        &[3.0, -3.0, 4.0, -4.0, 5.0, -5.0, 6.0, -6.0, 7.0, -7.0],
    );
}

#[test]
fn recorder_ignores_inactive_samples_without_skip_back() {
    let mut fixture = RecorderFixture::new();

    for sample in 1..=5 {
        let sample = f64::from(sample);
        fixture.render(1, sample, -sample);
    }

    fixture.set_active(true);
    fixture.render(1, 6.0, -6.0);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);

    let samples = RecorderFixture::read_wav(&files[0]);
    assert_samples(&samples, &[6.0, -6.0]);
}

#[test]
fn recorder_starts_cold_after_stopping() {
    let mut fixture = RecorderFixture::new_with_skip_back(3.0 / f64::from(fixture::SAMPLE_RATE));

    for sample in 1..=5 {
        let sample = f64::from(sample);
        fixture.render(1, sample, -sample);
    }

    fixture.set_active(true);
    fixture.render(1, 6.0, -6.0);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    // Only audio after the stop feeds the next take's pre-roll.
    for sample in 7..=8 {
        let sample = f64::from(sample);
        fixture.render(1, sample, -sample);
    }

    fixture.set_active(true);
    fixture.render(1, 9.0, -9.0);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 2);

    assert_samples(
        &RecorderFixture::read_wav(&files[0]),
        &[3.0, -3.0, 4.0, -4.0, 5.0, -5.0, 6.0, -6.0],
    );
    assert_samples(
        &RecorderFixture::read_wav(&files[1]),
        &[7.0, -7.0, 8.0, -8.0, 9.0, -9.0],
    );
}

#[test]
fn recorder_clears_skip_back_history_on_inactive_overflow() {
    let mut fixture = RecorderFixture::new_with_skip_back(3.0 / f64::from(fixture::SAMPLE_RATE));

    let max_frames = 10 * usize::try_from(fixture::SAMPLE_RATE).unwrap();
    for _ in 0..max_frames / fixture::BUFFER_SIZE + 1 {
        fixture.render(fixture::BUFFER_SIZE, 1.0, -1.0);
    }

    for _ in 0..1000 {
        fixture.poll();
    }

    fixture.set_active(true);
    fixture.render(1, 42.0, -42.0);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);

    let samples = RecorderFixture::read_wav(&files[0]);
    assert_samples(&samples, &[42.0, -42.0]);
}

#[test]
fn recorder_rounds_skip_back_duration_to_frames() {
    assert_samples(
        &record_with_skip_back_frames(2.4, 5, 6.0),
        &[4.0, -4.0, 5.0, -5.0, 6.0, -6.0],
    );
    assert_samples(
        &record_with_skip_back_frames(2.6, 5, 6.0),
        &[3.0, -3.0, 4.0, -4.0, 5.0, -5.0, 6.0, -6.0],
    );
}

#[test]
fn recorder_flushes_large_skip_back_history_across_polls() {
    const HISTORY_FRAMES: usize = 24_001;

    let mut fixture = RecorderFixture::new_with_skip_back(
        HISTORY_FRAMES as f64 / f64::from(fixture::SAMPLE_RATE),
    );

    for _ in 0..HISTORY_FRAMES / fixture::BUFFER_SIZE {
        fixture.render(fixture::BUFFER_SIZE, 1.0, -1.0);
    }
    let remainder = HISTORY_FRAMES % fixture::BUFFER_SIZE;
    if remainder > 0 {
        fixture.render(remainder, 1.0, -1.0);
    }

    fixture.set_active(true);
    fixture.render(1, 42.0, -42.0);

    for _ in 0..20 {
        fixture.poll();
    }

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);

    let samples = RecorderFixture::read_wav(&files[0]);
    assert_eq!(samples.len(), 2 * (HISTORY_FRAMES + 1));
    assert_samples(&samples[..4], &[1.0, -1.0, 1.0, -1.0]);
    assert_samples(&samples[samples.len() - 2..], &[42.0, -42.0]);
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

    let max_frames = 10 * usize::try_from(fixture::SAMPLE_RATE).unwrap();
    for _ in 0..max_frames / fixture::BUFFER_SIZE + 1 {
        fixture.render(fixture::BUFFER_SIZE, 0.25, 0.75);
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

fn record_with_skip_back_frames(
    skip_back_frames: f64,
    pre_frames: usize,
    live_sample: f64,
) -> Vec<f32> {
    let mut fixture =
        RecorderFixture::new_with_skip_back(skip_back_frames / f64::from(fixture::SAMPLE_RATE));

    for sample in 1..=pre_frames {
        let sample = sample as f64;
        fixture.render(1, sample, -sample);
    }

    fixture.set_active(true);
    fixture.render(1, live_sample, -live_sample);
    fixture.poll();

    fixture.set_active(false);
    fixture.render(0, 0.0, 0.0);
    fixture.poll();

    let files = fixture.files();
    assert_eq!(files.len(), 1);
    RecorderFixture::read_wav(&files[0])
}

fn assert_samples(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert_approx_eq!(actual, expected);
    }
}
