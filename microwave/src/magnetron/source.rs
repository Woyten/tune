use std::{
    fmt,
    marker::PhantomData,
    ops::{Add, Mul},
};

use magnetron::{
    automation::{Automation, AutomationContext},
    spec::{Creator, Spec},
};
use serde::{
    de::{self, value::MapAccessDeserializer, IntoDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};

use super::{
    oscillator::{OscillatorKind, OscillatorRunner},
    AutomationSpec,
};

pub trait StorageAccess: Clone + Send + 'static {
    type Storage;

    fn access(&mut self, storage: &Self::Storage) -> f64;
}

#[derive(Clone, Deserialize, Serialize)]
pub enum NoAccess {}

impl StorageAccess for NoAccess {
    type Storage = ();

    fn access(&mut self, _storage: &Self::Storage) -> f64 {
        unreachable!("NoControl is inhabitable")
    }
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum LfSource<P, C> {
    Value(f64),
    Property(P),
    Expr(Box<LfSourceExpr<P, C>>),
}

impl<'de, P: Deserialize<'de>, C: Deserialize<'de>> Deserialize<'de> for LfSource<P, C> {
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
struct LfSourceVisitor<P, C> {
    phantom: PhantomData<(P, C)>,
}

impl<'de, P: Deserialize<'de>, C: Deserialize<'de>> Visitor<'de> for LfSourceVisitor<P, C> {
    type Value = LfSource<P, C>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "float value, property or nested LF source expression"
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
        P::deserialize(v.into_deserializer()).map(LfSource::Property)
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
pub enum LfSourceExpr<P, C> {
    Add(LfSource<P, C>, LfSource<P, C>),
    Mul(LfSource<P, C>, LfSource<P, C>),
    Linear {
        input: LfSource<P, C>,
        from: LfSource<P, C>,
        to: LfSource<P, C>,
    },
    Oscillator {
        kind: OscillatorKind,
        frequency: LfSource<P, C>,
        phase: Option<LfSource<P, C>>,
        baseline: LfSource<P, C>,
        amplitude: LfSource<P, C>,
    },
    Time {
        start: LfSource<P, C>,
        end: LfSource<P, C>,
        from: LfSource<P, C>,
        to: LfSource<P, C>,
    },
    Controller {
        kind: C,
        from: LfSource<P, C>,
        to: LfSource<P, C>,
    },
}

impl<P, C> LfSourceExpr<P, C> {
    pub fn wrap(self) -> LfSource<P, C> {
        LfSource::Expr(Box::new(self))
    }
}

impl<P: StorageAccess, C: StorageAccess> Spec for LfSource<P, C> {
    type Created = Automation<(P::Storage, C::Storage)>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        match self {
            &LfSource::Value(constant) => creator.create_automation((), move |_, ()| constant),
            LfSource::Property(property) => {
                let mut property = property.clone();
                creator.create_automation(
                    (),
                    move |context: &AutomationContext<(P::Storage, C::Storage)>, ()| {
                        property.access(&context.payload.0)
                    },
                )
            }
            LfSource::Expr(expr) => match &**expr {
                LfSourceExpr::Add(a, b) => creator.create_automation((a, b), |_, (a, b)| a + b),
                LfSourceExpr::Mul(a, b) => creator.create_automation((a, b), |_, (a, b)| a * b),
                LfSourceExpr::Linear { input, from, to } => {
                    let mut value = creator.create(input);
                    create_scaled_value_automation(creator, from, to, move |context| {
                        context.read(&mut value)
                    })
                }
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
                LfSourceExpr::Time {
                    start,
                    end,
                    from,
                    to,
                } => {
                    let mut start_end = creator.create((start, end));
                    let mut secs_since_pressed = 0.0;
                    create_scaled_value_automation(creator, from, to, move |context| {
                        let curr_time = secs_since_pressed;
                        secs_since_pressed += context.render_window_secs;

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
                LfSourceExpr::Controller { kind, from, to } => {
                    let mut kind = kind.clone();
                    create_scaled_value_automation(creator, from, to, move |context| {
                        kind.access(&context.payload.1)
                    })
                }
            },
        }
    }
}

fn create_scaled_value_automation<P: StorageAccess, C: StorageAccess>(
    creator: &Creator,
    from: &LfSource<P, C>,
    to: &LfSource<P, C>,
    mut value_fn: impl FnMut(&AutomationContext<(P::Storage, C::Storage)>) -> f64 + Send + 'static,
) -> Automation<(P::Storage, C::Storage)> {
    creator.create_automation((from, to), move |context, (from, to)| {
        from + value_fn(context) * (to - from)
    })
}

struct LfSourceOscillatorRunner<'a, P, C> {
    creator: &'a Creator,
    frequency: &'a LfSource<P, C>,
    phase: &'a Option<LfSource<P, C>>,
    baseline: &'a LfSource<P, C>,
    amplitude: &'a LfSource<P, C>,
}

impl<P: StorageAccess, C: StorageAccess> OscillatorRunner for LfSourceOscillatorRunner<'_, P, C> {
    type Result = Automation<(P::Storage, C::Storage)>;

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

impl<P: StorageAccess, C: StorageAccess> AutomationSpec for LfSource<P, C> {
    type Context = (P::Storage, C::Storage);
}

impl<P, C> Add for LfSource<P, C> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Add(self, rhs).wrap()
    }
}

impl<P, C> Mul for LfSource<P, C> {
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
        magnetron::{StageSpec, WaveformProperty},
    };

    use super::*;

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
            payload: &(
                WaveformState {
                    pitch_hz: 0.0,
                    velocity: 0.0,
                    key_pressure: 0.0,
                    secs_since_pressed: 0.0,
                    secs_since_released: 0.0,
                },
                (),
            ),
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
            "Filter: invalid type: unit value, expected float value, property or nested LF source expression at line 3 column 7"
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
            "Filter: invalid type: integer `10000`, expected float value, property or nested LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_property() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to: InvalidProperty
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
           get_parse_error(yml),
            "Filter: unknown variant `InvalidProperty`, expected one of `WaveformPitch`, `WaveformPeriod`, `Velocity`, `KeyPressure` at line 3 column 7"
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
            "Filter: unknown variant `InvalidExpr`, expected one of `Add`, `Mul`, `Linear`, `Oscillator`, `Time`, `Controller` at line 3 column 7"
        )
    }

    fn parse_lf_source(lf_source: &str) -> LfSource<WaveformProperty, NoAccess> {
        serde_yaml::from_str(lf_source).unwrap()
    }

    fn get_parse_error(yml: &str) -> String {
        serde_yaml::from_str::<StageSpec<LfSource<WaveformProperty, LiveParameter>>>(yml)
            .err()
            .unwrap()
            .to_string()
    }
}
