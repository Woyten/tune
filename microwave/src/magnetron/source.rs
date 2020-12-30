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
    Expr(LfSourceExpr<C>),
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
        write!(formatter, "float value or LF source expression")
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
        LfSourceExpr::deserialize(v.into_deserializer()).map(LfSource::Expr)
    }

    // Handles the case where a struct variant is provided as an input source
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        LfSourceExpr::deserialize(MapAccessDeserializer::new(map)).map(LfSource::Expr)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceExpr<C> {
    Add(Box<LfSource<C>>, Box<LfSource<C>>),
    Mul(Box<LfSource<C>>, Box<LfSource<C>>),
    Time {
        start: Box<LfSource<C>>,
        end: Box<LfSource<C>>,
        from: Box<LfSource<C>>,
        to: Box<LfSource<C>>,
    },
    Oscillator {
        kind: OscillatorKind,
        phase: f64,
        frequency: Box<LfSource<C>>,
        baseline: Box<LfSource<C>>,
        amplitude: Box<LfSource<C>>,
    },
    Control {
        controller: C,
        from: Box<LfSource<C>>,
        to: Box<LfSource<C>>,
    },
    WaveformPitch,
}

impl<C> From<LfSourceExpr<C>> for LfSource<C> {
    fn from(v: LfSourceExpr<C>) -> Self {
        LfSource::Expr(v)
    }
}

impl<C: Controller> LfSource<C> {
    pub fn next(&mut self, control: &WaveformControl<C::Storage>) -> f64 {
        match self {
            LfSource::Value(constant) => *constant,
            LfSource::Expr(LfSourceExpr::Add(a, b)) => a.next(control) + b.next(control),
            LfSource::Expr(LfSourceExpr::Mul(a, b)) => a.next(control) * b.next(control),
            LfSource::Expr(LfSourceExpr::Time {
                start,
                end,
                from,
                to,
            }) => {
                let start = start.next(control);
                let end = end.next(control);
                let from = from.next(control);
                let to = to.next(control);

                let curr_time = control.total_secs;
                if curr_time <= start && curr_time <= end {
                    from
                } else if curr_time >= start && curr_time >= end {
                    to
                } else {
                    from + (to - from) * (control.total_secs - start) / (end - start)
                }
            }
            LfSource::Expr(LfSourceExpr::Oscillator {
                kind,
                phase,
                frequency,
                baseline,
                amplitude,
            }) => {
                let signal = match kind {
                    OscillatorKind::Sin => functions::sin(*phase),
                    OscillatorKind::Sin3 => functions::sin3(*phase),
                    OscillatorKind::Triangle => functions::triangle(*phase),
                    OscillatorKind::Square => functions::square(*phase),
                    OscillatorKind::Sawtooth => functions::sawtooth(*phase),
                };

                *phase = (*phase + frequency.next(control) * control.buffer_secs).rem_euclid(1.0);

                baseline.next(control) + signal * amplitude.next(control)
            }
            LfSource::Expr(LfSourceExpr::Control {
                controller,
                from,
                to,
            }) => {
                let from = from.next(control);
                let to = to.next(control);
                from + controller.read(&control.storage) * (to - from)
            }
            LfSource::Expr(LfSourceExpr::WaveformPitch) => control.pitch.as_hz(),
        }
    }
}

impl<C> Add for LfSource<C> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Add(self.into(), rhs.into()).into()
    }
}

impl<C> Mul for LfSource<C> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Mul(self.into(), rhs.into()).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{magnetron::waveform::StageSpec, synth::SynthControl};

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
  source: Buffer0
  destination:
    buffer: AudioOut
    intensity: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: invalid type: unit value, expected float value or LF source expression at line 3 column 7"
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
  source: Buffer0
  destination:
    buffer: AudioOut
    intensity: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: invalid type: integer `10000`, expected float value or LF source expression at line 3 column 7"
        )
    }

    #[test]
    fn deserialize_stage_with_invalid_lf_source() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to: Invalid
  quality: 5.0
  source: Buffer0
  destination:
    buffer: AudioOut
    intensity: 1.0";
        assert_eq!(
            serde_yaml::from_str::<StageSpec<SynthControl>>(yml)
                .err()
                .unwrap()
                .to_string(),
            "Filter: unknown variant `Invalid`, expected one of `Add`, `Mul`, `Time`, `Oscillator`, `Control`, `Property`, `WaveformPitch` at line 3 column 7"
        )
    }
}
