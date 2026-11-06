use std::sync::Arc;

use crate::app::VisualizerMode;
use crate::app::VisualizerState;
use crate::toggle::Direction;
use crate::visualizer::VisualizerControl;

#[test]
fn visualizer_state_source_mapping() {
    let control = Arc::new(VisualizerControl::new());
    let mut state = VisualizerState::new(vec!["a".into(), "b".into()], control);

    assert_eq!(state.mode(), VisualizerMode::Oscilloscope);
    assert_eq!(state.source_index(), None);
    assert_eq!(state.source_label(), "Output (stereo)");

    state.switch_source(Direction::Forward);
    assert_eq!(state.source_index(), Some(0));
    assert_eq!(state.source_label(), "a");

    state.switch_source(Direction::Forward);
    assert_eq!(state.source_index(), Some(1));
    assert_eq!(state.source_label(), "b");

    state.switch_source(Direction::Forward);
    assert_eq!(state.source_index(), Some(1));

    state.switch_source(Direction::Backward);
    assert_eq!(state.source_index(), Some(0));

    state.switch_source(Direction::Backward);
    assert_eq!(state.source_index(), None);
}

#[test]
fn visualizer_state_mode_switching() {
    let control = Arc::new(VisualizerControl::new());
    let mut state = VisualizerState::new(vec![], control);

    assert_eq!(state.mode(), VisualizerMode::Oscilloscope);

    state.switch_mode(Direction::Forward);
    assert_eq!(state.mode(), VisualizerMode::Spectrograph);

    state.switch_mode(Direction::Forward);
    assert_eq!(state.mode(), VisualizerMode::OscilloscopeAndSpectrograph);

    state.switch_mode(Direction::Forward);
    assert_eq!(state.mode(), VisualizerMode::OscilloscopeAndSpectrograph);

    state.switch_mode(Direction::Backward);
    assert_eq!(state.mode(), VisualizerMode::Spectrograph);
}
