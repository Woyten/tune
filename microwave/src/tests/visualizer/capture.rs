use crate::tests::visualizer::fixture::VisualizerFixture;
use crate::visualizer::CAPTURE_CAPACITY_FRAMES;
use crate::visualizer::HISTORY_FRAMES;

#[test]
fn capture_preserves_channel_order() {
    let mut fixture = VisualizerFixture::new(48_000);

    fixture.push_stereo(&[0.25, -0.5], &[0.75, -1.0]);
    fixture.update();

    assert_eq!(fixture.samples(), [0.25, 0.75, -0.5, -1.0]);
}

#[test]
fn capture_tracks_absolute_start_frame() {
    let mut fixture = VisualizerFixture::new(48_000);
    let frames = HISTORY_FRAMES + 10;
    let samples = (0..frames).map(|frame| frame as f64).collect::<Vec<_>>();

    fixture.push_stereo(&samples, &vec![0.0; frames]);
    fixture.update();

    assert_eq!(fixture.start_frame(), 10);
    assert_eq!(fixture.samples().len(), 2 * HISTORY_FRAMES);
}

#[test]
fn capture_update_reports_new_frames_once() {
    let mut fixture = VisualizerFixture::new(48_000);

    fixture.push_stereo(&[0.0, 0.1, 0.2], &[0.0, 0.1, 0.2]);
    assert_eq!(fixture.update().new_frames, 3);
    assert_eq!(fixture.update().new_frames, 0);

    fixture.push_stereo(&[0.3, 0.4], &[0.3, 0.4]);
    assert_eq!(fixture.update().new_frames, 2);

    fixture.clear_history();
    assert_eq!(fixture.update().new_frames, 0);
}

#[test]
fn capture_reports_gap_and_resumes_after_overflow() {
    let mut fixture = VisualizerFixture::new(48_000);
    let frames = CAPTURE_CAPACITY_FRAMES + 10;
    let samples = vec![0.5; frames];

    fixture.push_stereo(&samples, &samples);
    let update = fixture.update();

    assert!(update.gap);
    assert!(fixture.samples().is_empty());
    assert_eq!(fixture.start_frame(), CAPTURE_CAPACITY_FRAMES as u64);

    fixture.push_stereo(&[0.25, 0.25], &[0.75, 0.75]);
    let update = fixture.update();

    assert!(!update.gap);
    assert_eq!(update.new_frames, 2);
    assert_eq!(fixture.samples(), [0.25, 0.75, 0.25, 0.75]);
}

#[test]
fn capture_clear_advances_start_frame() {
    let mut fixture = VisualizerFixture::new(48_000);
    let samples = vec![0.5; 100];

    fixture.push_stereo(&samples, &samples);
    fixture.update();
    assert_eq!(fixture.start_frame(), 0);

    fixture.clear_history();
    assert!(fixture.samples().is_empty());
    assert_eq!(fixture.start_frame(), 100);

    fixture.push_stereo(&[0.25], &[0.75]);
    fixture.update();
    assert_eq!(fixture.start_frame(), 100);
    assert_eq!(fixture.samples(), [0.25, 0.75]);
}
