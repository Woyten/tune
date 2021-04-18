use std::{
    fmt,
    marker::PhantomData,
    ops::{Add, Mul},
};

use serde::{
    de::{self, value::MapAccessDeserializer, IntoDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};

use super::{control::Controller, functions, oscillator::OscillatorKind, WaveformControl};

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
        LfSourceUnit::deserialize(v.into_deserializer()).map(Into::into)
    }

    // Handles the case where a struct variant is provided as an input source
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        LfSourceExpr::deserialize(MapAccessDeserializer::new(map)).map(Into::into)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceUnit {
    WaveformPitch,
    Wavelength,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceExpr<C> {
    Add(LfSource<C>, LfSource<C>),
    Mul(LfSource<C>, LfSource<C>),
    Oscillator {
        kind: OscillatorKind,
        phase: f64,
        frequency: LfSource<C>,
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
    Property {
        kind: Property,
        from: LfSource<C>,
        to: LfSource<C>,
    },
    Control {
        controller: C,
        from: LfSource<C>,
        to: LfSource<C>,
    },
}

impl<C> From<LfSourceUnit> for LfSource<C> {
    fn from(unit: LfSourceUnit) -> Self {
        LfSource::Unit(unit)
    }
}

impl<C> From<LfSourceExpr<C>> for LfSource<C> {
    fn from(expr: LfSourceExpr<C>) -> Self {
        LfSource::Expr(Box::new(expr))
    }
}

impl<C: Controller> LfSource<C> {
    pub fn next(&mut self, control: &WaveformControl<C::Storage>) -> f64 {
        match self {
            LfSource::Value(constant) => *constant,
            LfSource::Unit(unit) => match unit {
                LfSourceUnit::WaveformPitch => {
                    (control.properties.pitch * control.properties.pitch_bend).as_hz()
                }
                LfSourceUnit::Wavelength => {
                    1.0 / (control.properties.pitch * control.properties.pitch_bend).as_hz()
                }
            },
            LfSource::Expr(expr) => match &mut **expr {
                LfSourceExpr::Add(a, b) => a.next(control) + b.next(control),
                LfSourceExpr::Mul(a, b) => a.next(control) * b.next(control),
                LfSourceExpr::Oscillator {
                    kind,
                    phase,
                    frequency,
                    baseline,
                    amplitude,
                } => {
                    let frequency = frequency.next(control);
                    let baseline = baseline.next(control);
                    let amplitude = amplitude.next(control);

                    let signal = match kind {
                        OscillatorKind::Sin => functions::sin(*phase),
                        OscillatorKind::Sin3 => functions::sin3(*phase),
                        OscillatorKind::Triangle => functions::triangle(*phase),
                        OscillatorKind::Square => functions::square(*phase),
                        OscillatorKind::Sawtooth => functions::sawtooth(*phase),
                    };

                    *phase = (*phase + frequency * control.buffer_secs).rem_euclid(1.0);

                    baseline + signal * amplitude
                }
                LfSourceExpr::Envelope { name, from, to } => {
                    let from = from.next(control);
                    let to = to.next(control);

                    let envelope_value = control.envelope_map[name].create_envelope().get_value(
                        control.properties.secs_since_pressed,
                        control.properties.secs_since_released,
                    );

                    from + envelope_value * (to - from)
                }
                LfSourceExpr::Time {
                    start,
                    end,
                    from,
                    to,
                } => {
                    let start = start.next(control);
                    let end = end.next(control);
                    let from = from.next(control);
                    let to = to.next(control);

                    let curr_time = control.properties.secs_since_pressed;
                    if curr_time <= start && curr_time <= end {
                        from
                    } else if curr_time >= start && curr_time >= end {
                        to
                    } else {
                        from + (to - from) * (curr_time - start) / (end - start)
                    }
                }
                LfSourceExpr::Property { kind, from, to } => {
                    let from = from.next(control);
                    let to = to.next(control);

                    let value = match kind {
                        Property::Velocity => control.properties.velocity,
                        Property::KeyPressure => control.properties.pressure,
                    };

                    from + value * (to - from)
                }
                LfSourceExpr::Control {
                    controller,
                    from,
                    to,
                } => {
                    let from = from.next(control);
                    let to = to.next(control);

                    from + controller.read(&control.storage) * (to - from)
                }
            },
        }
    }
}

impl<C> Add for LfSource<C> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Add(self, rhs).into()
    }
}

impl<C> Mul for LfSource<C> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Mul(self, rhs).into()
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Property {
    Velocity,
    KeyPressure,
}

#[cfg(test)]
mod tests {
    use crate::{magnetron::spec::StageSpec, synth::SynthControl};

    #[test]
    fn deserialize_stage_with_missing_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to:
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: invalid type: unit value, expected float value, unit expression or nested LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_integer_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to: 10000
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: invalid type: integer `10000`, expected float value, unit expression or nested LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_unit_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to: InvalidUnit
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: unknown variant `InvalidUnit`, expected `WaveformPitch` or `Wavelength` at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_lf_source_expression() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to:
        InvalidExpr:
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: unknown variant `InvalidExpr`, expected one of `Add`, `Mul`, `Oscillator`, `Envelope`, `Time`, `Property`, `Control` at line 3 column 7"
        )
    }
}
