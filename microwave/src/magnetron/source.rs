use std::{
    collections::HashMap,
    fmt,
    marker::PhantomData,
    ops::{Add, Mul},
};

use magnetron::automation::{
    Automatable, Automated, AutomatedValue, AutomationFactory, CreationInfo, QueryInfo,
    RenderWindowSecs,
};
use serde::{
    de::{self, value::MapAccessDeserializer, IntoDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};
use tune::pitch::Ratio;

use super::{
    oscillator::{OscillatorRunner, OscillatorType},
    AutomatableParam,
};

pub trait StorageAccess: Clone + Send + 'static {
    type Storage;

    fn access(&mut self, storage: &Self::Storage) -> f64;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum NoAccess {}

impl StorageAccess for NoAccess {
    type Storage = ();

    fn access(&mut self, _storage: &Self::Storage) -> f64 {
        unreachable!("NoControl is inhabitable")
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum LfSource<P, C> {
    Value(f64),
    Template(String),
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
        String::deserialize(v.into_deserializer()).map(LfSource::Template)
    }

    // Handles the case where a struct variant is provided as an input source
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        LfSourceExpr::deserialize(MapAccessDeserializer::new(map)).map(LfSourceExpr::wrap)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LfSourceExpr<P, C> {
    Add(LfSource<P, C>, LfSource<P, C>),
    Mul(LfSource<P, C>, LfSource<P, C>),
    Linear {
        input: LfSource<P, C>,
        map0: LfSource<P, C>,
        map1: LfSource<P, C>,
    },
    Oscillator {
        kind: OscillatorType,
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
    Fader {
        movement: LfSource<P, C>,
        map0: LfSource<P, C>,
        map1: LfSource<P, C>,
    },
    Semitones(LfSource<P, C>),
    Global(String),
    Property(P),
    Controller {
        kind: C,
        map0: LfSource<P, C>,
        map1: LfSource<P, C>,
    },
}

impl<P, C> LfSource<P, C> {
    pub fn template(template_name: &str) -> LfSource<P, C> {
        LfSource::Template(template_name.to_owned())
    }
}

impl<P, C> LfSourceExpr<P, C> {
    pub fn wrap(self) -> LfSource<P, C> {
        LfSource::Expr(Box::new(self))
    }
}

impl<P: StorageAccess, C: StorageAccess> CreationInfo for LfSource<P, C> {
    type Context = HashMap<String, Self>;
}

impl<P: StorageAccess, C: StorageAccess> QueryInfo for LfSource<P, C> {
    type Context<'a> = (&'a P::Storage, &'a C::Storage, &'a HashMap<String, f64>);
}

impl<P: StorageAccess, C: StorageAccess> Automatable<LfSource<P, C>> for LfSource<P, C> {
    type Output = AutomatedValue<Self>;

    fn create(&self, factory: &mut AutomationFactory<Self>) -> Self::Output {
        match self {
            &LfSource::Value(constant) => {
                factory.automate(()).into_automation(move |_, ()| constant)
            }
            LfSource::Template(template_name) => {
                match factory.context_mut().remove_entry(template_name) {
                    Some((template_name, template)) => {
                        let created = factory.automate(&template);
                        factory.context_mut().insert(template_name, template);
                        created
                    }
                    None => {
                        log::warn!("Unknown or nested template {template_name}");
                        factory.automate(()).into_automation(|_, _| 0.0)
                    }
                }
            }
            LfSource::Expr(expr) => match &**expr {
                LfSourceExpr::Add(a, b) => {
                    factory.automate((a, b)).into_automation(|_, (a, b)| a + b)
                }
                LfSourceExpr::Mul(a, b) => {
                    factory.automate((a, b)).into_automation(|_, (a, b)| a * b)
                }
                LfSourceExpr::Linear { input, map0, map1 } => {
                    create_scaled_value_automation(factory, input, map0, map1, move |_, input| {
                        input
                    })
                }
                LfSourceExpr::Oscillator {
                    kind,
                    frequency,
                    phase,
                    baseline,
                    amplitude,
                } => kind.run_oscillator(LfSourceOscillatorRunner {
                    factory,
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
                } => create_scaled_value_automation(
                    factory,
                    &(RenderWindowSecs, start, end),
                    from,
                    to,
                    {
                        let mut secs_since_pressed = 0.0;
                        move |_, (render_window_secs, start, end)| {
                            let curr_time = secs_since_pressed;
                            secs_since_pressed += render_window_secs;

                            if curr_time <= start && curr_time <= end {
                                0.0
                            } else if curr_time >= start && curr_time >= end {
                                1.0
                            } else {
                                (curr_time - start) / (end - start)
                            }
                        }
                    },
                ),
                LfSourceExpr::Fader {
                    movement,
                    map0,
                    map1,
                } => create_scaled_value_automation(
                    factory,
                    &(RenderWindowSecs, movement),
                    map0,
                    map1,
                    {
                        let mut curr_position = 0.0;
                        move |_, (render_window_secs, movement)| {
                            let result = curr_position;
                            curr_position = (curr_position + movement * render_window_secs)
                                .max(0.0)
                                .min(1.0);
                            result
                        }
                    },
                ),
                LfSourceExpr::Semitones(semitones) => factory
                    .automate(semitones)
                    .into_automation(|_, semitones| Ratio::from_semitones(semitones).as_float()),
                LfSourceExpr::Global(name) => {
                    let name = name.clone();

                    factory.automate(()).into_automation(
                        move |(_, _, globals): (_, _, &HashMap<String, f64>), ()| {
                            globals.get(&name).copied().unwrap_or_default()
                        },
                    )
                }
                LfSourceExpr::Property(kind) => {
                    let mut kind = kind.clone();
                    factory.automate(()).into_automation(
                        move |(properties, _, _): (_, _, _), ()| kind.access(properties),
                    )
                }
                LfSourceExpr::Controller { kind, map0, map1 } => {
                    let mut kind = kind.clone();
                    create_scaled_value_automation(
                        factory,
                        &(),
                        map0,
                        map1,
                        move |(_, controllers, _), ()| kind.access(controllers),
                    )
                }
            },
        }
    }
}

fn create_scaled_value_automation<T, A>(
    factory: &mut AutomationFactory<A>,
    automatable: &T,
    map0: &A,
    map1: &A,
    mut value_fn: impl FnMut(<A as QueryInfo>::Context<'_>, <T::Output as Automated<A>>::Output<'_>) -> f64
        + Send
        + 'static,
) -> AutomatedValue<A>
where
    T: Automatable<A>,
    T::Output: Automated<A> + Send + 'static,
    A: AutomatableParam,
{
    factory.automate((automatable, map0, map1)).into_automation(
        move |context, (value, from, to)| from + value_fn(context, value) * (to - from),
    )
}

struct LfSourceOscillatorRunner<'a, A: AutomatableParam> {
    factory: &'a mut AutomationFactory<A>,
    frequency: &'a A,
    phase: &'a Option<A>,
    baseline: &'a A,
    amplitude: &'a A,
}

impl<A: AutomatableParam> OscillatorRunner for LfSourceOscillatorRunner<'_, A> {
    type Result = AutomatedValue<A>;

    fn apply_oscillator_fn(
        &mut self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result {
        let mut last_phase = 0.0;
        let mut total_phase = 0.0;
        self.factory
            .automate((
                RenderWindowSecs,
                (self.phase, self.frequency),
                (self.baseline, self.amplitude),
            ))
            .into_automation(
                move |_, (render_window_secs, (phase, frequency), (baseline, amplitude))| {
                    let phase = phase.unwrap_or_default();
                    total_phase = (total_phase + phase - last_phase).rem_euclid(1.0);
                    last_phase = phase;
                    let signal = oscillator_fn(total_phase);
                    total_phase += frequency * render_window_secs;
                    baseline + signal * amplitude
                },
            )
    }
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
    use std::f64::consts::TAU;

    use assert_approx_eq::assert_approx_eq;

    use crate::{
        magnetron::{
            filter::{FilterSpec, FilterType},
            waveform::WaveformProperties,
            ProcessorType,
        },
        profile::WaveformAutomatableValue,
    };

    use super::*;

    #[test]
    fn lf_source_oscillator_correctness() {
        let mut factory = AutomationFactory::new(HashMap::new());
        let lf_source = parse_lf_source(
            r"
Oscillator:
  kind: Sin
  frequency: 440.0
  phase: 0.25
  baseline: 0.0
  amplitude: 1.0",
        );

        let mut automation = factory.automate(lf_source);

        let render_window_secs = 1.0 / 100.0;
        let context = (
            &WaveformProperties::initial(0.0, 0.0),
            &Default::default(),
            &HashMap::new(),
        );

        assert_approx_eq!(
            automation.query(render_window_secs, context),
            (0.0 * TAU).cos()
        );
        assert_approx_eq!(
            automation.query(render_window_secs, context),
            (0.4 * TAU).cos()
        );
        assert_approx_eq!(
            automation.query(render_window_secs, context),
            (0.8 * TAU).cos()
        );
        assert_approx_eq!(
            automation.query(render_window_secs, context),
            (0.2 * TAU).cos()
        );
    }

    #[test]
    fn deserialize_stage_with_missing_lf_source() {
        let yml = r"
in_buffer: 0
out_buffer: 7
processor_type: Filter
filter_type: LowPass2
resonance:
  Controller:
    kind: Modulation
    map0: 0.0
    map1:
quality: 5.0";
        assert_eq!(
            get_parse_error(yml),
            "invalid type: unit value, expected float value, property or nested LF source expression"
        )
    }

    #[test]
    fn deserialize_stage_with_integer_lf_source() {
        let yml = r"
in_buffer: 0
out_buffer: 7
processor_type: Filter
filter_type: LowPass2
resonance:
  Controller:
    kind: Modulation
    map0: 0.0
    map1: 10000
quality: 5.0";
        assert_eq!(
           get_parse_error(yml),
            "invalid type: integer `10000`, expected float value, property or nested LF source expression"
        )
    }

    #[test]
    fn deserialize_stage_with_template() {
        let yml = r"
in_buffer: 0
out_buffer: 5
processor_type: Filter
filter_type: LowPass2
resonance:
  Controller:
    kind: Modulation
    map0: 0.0
    map1: AnyNameWorks
quality: 5.0";

        let expr = if let ProcessorType::Filter(FilterSpec {
            filter_type:
                FilterType::LowPass2 {
                    resonance: LfSource::Expr(expr),
                    ..
                },
            ..
        }) = parse_stage(yml)
        {
            expr
        } else {
            panic!()
        };

        let template_name = if let LfSourceExpr::Controller {
            map1: LfSource::Template(template_name),
            ..
        } = *expr
        {
            template_name
        } else {
            panic!()
        };

        assert_eq!(template_name, "AnyNameWorks")
    }

    #[test]
    fn deserialize_stage_with_invalid_lf_source_expression() {
        let yml = r"
in_buffer: 0
out_buffer: 5
processor_type: Filter
filter_type: LowPass2
resonance:
  Controller:
    kind: Modulation
    map0: 0.0
    map1:
      InvalidExpr:
quality: 5.0";
        assert_eq!(
           get_parse_error(yml),
            "unknown variant `InvalidExpr`, expected one of `Add`, `Mul`, `Linear`, `Oscillator`, `Time`, `Fader`, `Semitones`, `Global`, `Property`, `Controller`"
        )
    }

    fn parse_lf_source(lf_source: &str) -> WaveformAutomatableValue {
        serde_yaml::from_str(lf_source).unwrap()
    }

    fn parse_stage(yml: &str) -> ProcessorType<WaveformAutomatableValue> {
        serde_yaml::from_str(yml).unwrap()
    }

    fn get_parse_error(yml: &str) -> String {
        serde_yaml::from_str::<ProcessorType<WaveformAutomatableValue>>(yml)
            .err()
            .unwrap()
            .to_string()
    }
}
