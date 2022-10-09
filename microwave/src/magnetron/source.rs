use std::{
    fmt,
    marker::PhantomData,
    ops::{Add, Mul},
};

use magnetron::{
    automation::{Automation, AutomationContext, AutomationSpec},
    spec::{Creator, Spec},
};
use serde::{
    de::{self, value::MapAccessDeserializer, IntoDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};

use super::{
    oscillator::{OscillatorKind, OscillatorRunner},
    WaveformStateAndStorage,
};

pub trait Controller: Clone + Send + 'static {
    type Storage;

    fn access(&mut self, storage: &Self::Storage) -> f64;
}

#[derive(Clone, Deserialize, Serialize)]
pub enum NoControl {}

impl Controller for NoControl {
    type Storage = ();

    fn access(&mut self, _storage: &Self::Storage) -> f64 {
        unreachable!("NoControl is inhabitable")
    }
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum LfSource<C> {
    Value(f64),
    Unit(LfSourceUnit),
    Expr(Box<LfSourceExpr<C>>),
}

impl<'de, C: Deserialize<'de>> Deserialize<'de> for LfSource<C> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(LfSourceVisitor {
            phantom: Default::default(),
        })
    }
}

// Visitor compensating for poor error messages when using untagged enums.
struct LfSourceVisitor<C> {
    phantom: PhantomData<C>,
}

impl<'de, C: Deserialize<'de>> Visitor<'de> for LfSourceVisitor<C> {
    type Value = LfSource<C>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "float value, unit expression or nested LF source expression"
        )
    }

    // Handles the case where a number is provided as an input source
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(LfSource::Value(v))
    }

    // Handles the case where a unit variant is provided as an input source
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        LfSourceUnit::deserialize(v.into_deserializer()).map(LfSourceUnit::wrap)
    }

    // Handles the case where a struct variant is provided as an input source
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        LfSourceExpr::deserialize(MapAccessDeserializer::new(map)).map(LfSourceExpr::wrap)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceUnit {
    WaveformPitch,
    WaveformPeriod,
}

impl LfSourceUnit {
    pub fn wrap<C>(self) -> LfSource<C> {
        LfSource::Unit(self)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceExpr<C> {
    Add(LfSource<C>, LfSource<C>),
    Mul(LfSource<C>, LfSource<C>),
    Oscillator {
        kind: OscillatorKind,
        frequency: LfSource<C>,
        phase: Option<LfSource<C>>,
        baseline: LfSource<C>,
        amplitude: LfSource<C>,
    },
    Envelope {
        name: String,
        from: LfSource<C>,
        to: LfSource<C>,
    },
    Time {
        start: LfSource<C>,
        end: LfSource<C>,
        from: LfSource<C>,
        to: LfSource<C>,
    },
    Velocity {
        from: LfSource<C>,
        to: LfSource<C>,
    },
    Controller {
        kind: C,
        from: LfSource<C>,
        to: LfSource<C>,
    },
}

impl<C> LfSourceExpr<C> {
    pub fn wrap(self) -> LfSource<C> {
        LfSource::Expr(Box::new(self))
    }
}

impl<C: Controller> Spec for LfSource<C> {
    type Created = Automation<WaveformStateAndStorage<C::Storage>>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        match self {
            &LfSource::Value(constant) => creator.create_automation(
                PhantomData::<WaveformStateAndStorage<C::Storage>>,
                move |_, ()| constant,
            ),
            LfSource::Unit(unit) => match unit {
                LfSourceUnit::WaveformPitch => creator.create_automation(
                    PhantomData::<WaveformStateAndStorage<C::Storage>>,
                    move |context, ()| context.payload.state.pitch_hz,
                ),
                LfSourceUnit::WaveformPeriod => creator.create_automation(
                    PhantomData::<WaveformStateAndStorage<C::Storage>>,
                    move |context, ()| context.payload.state.pitch_hz.recip(),
                ),
            },
            LfSource::Expr(expr) => match &**expr {
                LfSourceExpr::Add(a, b) => creator.create_automation((a, b), |_, (a, b)| a + b),
                LfSourceExpr::Mul(a, b) => creator.create_automation((a, b), |_, (a, b)| a * b),
                LfSourceExpr::Oscillator {
                    kind,
                    frequency,
                    phase,
                    baseline,
                    amplitude,
                } => kind.run_oscillator(LfSourceOscillatorRunner {
                    creator,
                    frequency,
                    phase,
                    baseline,
                    amplitude,
                }),
                LfSourceExpr::Envelope { name, from, to } => {
                    let envelope = creator.create_envelope(name).unwrap();
                    create_scaled_value_automation(creator, from, to, move |context| {
                        envelope.get_value(
                            context.payload.state.secs_since_pressed,
                            context.payload.state.secs_since_released,
                        )
                    })
                }
                LfSourceExpr::Time {
                    start,
                    end,
                    from,
                    to,
                } => {
                    let mut start_end = creator.create((start, end));
                    create_scaled_value_automation(creator, from, to, move |context| {
                        let curr_time = context.payload.state.secs_since_pressed;
                        let (start, end) = context.read(&mut start_end);

                        if curr_time <= start && curr_time <= end {
                            0.0
                        } else if curr_time >= start && curr_time >= end {
                            1.0
                        } else {
                            (curr_time - start) / (end - start)
                        }
                    })
                }
                LfSourceExpr::Velocity { from, to } => {
                    create_scaled_value_automation(creator, from, to, |context| {
                        context.payload.state.velocity
                    })
                }
                LfSourceExpr::Controller { kind, from, to } => {
                    let mut kind = kind.clone();
                    create_scaled_value_automation(creator, from, to, move |context| {
                        kind.access(&context.payload.storage)
                    })
                }
            },
        }
    }
}

fn create_scaled_value_automation<C: Controller>(
    creator: &Creator,
    from: &LfSource<C>,
    to: &LfSource<C>,
    mut value_fn: impl FnMut(&AutomationContext<WaveformStateAndStorage<C::Storage>>) -> f64
        + Send
        + 'static,
) -> Automation<WaveformStateAndStorage<C::Storage>> {
    creator.create_automation((from, to), move |context, (from, to)| {
        from + value_fn(context) * (to - from)
    })
}

struct LfSourceOscillatorRunner<'a, C> {
    creator: &'a Creator,
    frequency: &'a LfSource<C>,
    phase: &'a Option<LfSource<C>>,
    baseline: &'a LfSource<C>,
    amplitude: &'a LfSource<C>,
}

impl<C: Controller> OscillatorRunner for LfSourceOscillatorRunner<'_, C> {
    type Result = Automation<WaveformStateAndStorage<C::Storage>>;

    fn apply_oscillator_fn(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result {
        let mut last_phase = 0.0;
        let mut total_phase = 0.0;
        self.creator.create_automation(
            (
                (self.phase, self.frequency),
                (self.baseline, self.amplitude),
            ),
            move |context, ((phase, frequency), (baseline, amplitude))| {
                let phase = phase.unwrap_or_default();
                total_phase = (total_phase + phase - last_phase).rem_euclid(1.0);
                last_phase = phase;
                let signal = oscillator_fn(total_phase);
                total_phase += frequency * context.render_window_secs;
                baseline + signal * amplitude
            },
        )
    }
}

impl<C: Controller> AutomationSpec for LfSource<C> {
    type Context = WaveformStateAndStorage<C::Storage>;
}

impl<C> Add for LfSource<C> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Add(self, rhs).wrap()
    }
}

impl<C> Mul for LfSource<C> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Mul(self, rhs).wrap()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, f64::consts::TAU};

    use assert_approx_eq::assert_approx_eq;
    use magnetron::{automation::AutomationContext, spec::Creator, waveform::WaveformState};

    use crate::{
        control::LiveParameter,
        magnetron::{StageSpec, WaveformStateAndStorage},
    };

    use super::{LfSource, NoControl};

    #[test]
    fn lf_source_oscillator_correctness() {
        let creator = Creator::new(HashMap::new());
        let lf_source = parse_lf_source(
            r"
Oscillator:
  kind: Sin
  frequency: 440.0
  phase: 0.25
  baseline: 0.0
  amplitude: 1.0",
        );

        let mut automation = creator.create(lf_source);

        let context = AutomationContext {
            render_window_secs: 1.0 / 100.0,
            payload: &WaveformStateAndStorage {
                state: WaveformState {
                    pitch_hz: 0.0,
                    velocity: 0.0,
                    secs_since_pressed: 0.0,
                    secs_since_released: 0.0,
                },
                storage: (),
            },
        };

        assert_approx_eq!(context.read(&mut automation), (0.0 * TAU).cos());
        assert_approx_eq!(context.read(&mut automation), (0.4 * TAU).cos());
        assert_approx_eq!(context.read(&mut automation), (0.8 * TAU).cos());
        assert_approx_eq!(context.read(&mut automation), (0.2 * TAU).cos());
    }

    #[test]
    fn deserialize_stage_with_missing_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to:
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
            get_parse_error(yml),
            "Filter: invalid type: unit value, expected float value, unit expression or nested LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_integer_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to: 10000
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
           get_parse_error(yml),
            "Filter: invalid type: integer `10000`, expected float value, unit expression or nested LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_unit_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to: InvalidUnit
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
           get_parse_error(yml),
            "Filter: unknown variant `InvalidUnit`, expected `WaveformPitch` or `WaveformPeriod` at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_lf_source_expression() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to:
        InvalidExpr:
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
           get_parse_error(yml),
            "Filter: unknown variant `InvalidExpr`, expected one of `Add`, `Mul`, `Oscillator`, `Envelope`, `Time`, `Velocity`, `Controller` at line 3 column 7"
        )
    }

    fn parse_lf_source(lf_source: &str) -> LfSource<NoControl> {
        serde_yaml::from_str(lf_source).unwrap()
    }

    fn get_parse_error(yml: &str) -> String {
        serde_yaml::from_str::<StageSpec<LfSource<LiveParameter>>>(yml)
            .err()
            .unwrap()
            .to_string()
    }
}
