use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::str::FromStr;

use crate::key::PianoKey;
use crate::pitch::Pitch;
use crate::scala::Kbm;
use crate::scala::KbmBuildError;
use crate::scala::KbmRoot;
use crate::scala::Scl;
use crate::scala::SclBuildError;

pub(crate) fn import_scl(reader: impl Read) -> Result<Scl, SclImportError> {
    let mut lines = lines(reader);

    let (_, description) = next(&mut lines, SclStructuralError::ExpectingDescription)??;

    let (line_number, line) = next(&mut lines, SclStructuralError::ExpectingNumberOfNotes)??;
    let num_notes: u16 = parse(line_number, &line, SclParseErrorKind::IntValue)?;

    let mut builder = Scl::builder();
    while let Some((line_number, line)) = lines.next().transpose()? {
        let main_item = main_item(&line);
        if main_item.contains('.') {
            let cents_value = parse(line_number, main_item, SclParseErrorKind::CentsValue)?;
            builder = builder.push_cents(cents_value);
        } else if let Some((numer, denom)) = main_item.split_once('/') {
            let numer = parse(line_number, numer, SclParseErrorKind::Numer)?;
            let denom = parse(line_number, denom, SclParseErrorKind::Denom)?;
            builder = builder.push_fraction(numer, denom);
        } else {
            let int_value = parse(line_number, main_item, SclParseErrorKind::IntValue)?;
            builder = builder.push_int(int_value);
        }
    }

    let scl = builder.build_with_description(description)?;
    if scl.num_items() == num_notes {
        Ok(scl)
    } else {
        Err(SclStructuralError::InconsistentNumberOfNotes.into())
    }
}

/// Error reported when importing an [`Scl`] fails.
#[derive(Debug)]
pub enum SclImportError {
    IoError(io::Error),
    ParseError {
        line_number: usize,
        kind: SclParseErrorKind,
    },
    StructuralError(SclStructuralError),
    BuildError(SclBuildError),
}

/// Specifies which kind of item is suspected to be malformed.
#[derive(Clone, Debug)]
pub enum SclParseErrorKind {
    /// Invalid integer value or out of range.
    IntValue,

    /// Invalid cents value.
    CentsValue,

    /// Invalid numerator.
    Numer,

    /// Invalid denominator.
    Denom,
}

/// Indicates that the structure of the imported [`Scl`] file is incomplete.
#[derive(Clone, Debug)]
pub enum SclStructuralError {
    ExpectingDescription,
    ExpectingNumberOfNotes,
    InconsistentNumberOfNotes,
}

impl From<io::Error> for SclImportError {
    fn from(v: io::Error) -> Self {
        Self::IoError(v)
    }
}

impl From<ParseError<SclParseErrorKind>> for SclImportError {
    fn from(ParseError(line_number, kind): ParseError<SclParseErrorKind>) -> Self {
        Self::ParseError { line_number, kind }
    }
}

impl From<SclStructuralError> for SclImportError {
    fn from(v: SclStructuralError) -> Self {
        Self::StructuralError(v)
    }
}

impl From<SclBuildError> for SclImportError {
    fn from(v: SclBuildError) -> Self {
        Self::BuildError(v)
    }
}

pub(crate) fn import_kbm(reader: impl Read) -> Result<Kbm, KbmImportError> {
    let mut lines = lines(reader);

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingMapSize)??;
    let num_items: u16 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingFirstMidiNote)??;
    let first_midi: i32 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;
    let range_start = PianoKey::from_midi_number(first_midi);

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingLastMidiNote)??;
    let last_midi: i32 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;
    let range = range_start..PianoKey::from_midi_number(last_midi).plus_steps(1);

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingOrigin)??;
    let origin: i16 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingReferenceNote)??;
    let ref_note: i16 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingReferencePitch)??;
    let ref_pitch: f64 = parse(line_number, &line, KbmParseErrorKind::FloatValue)?;

    let kbm_root = KbmRoot {
        ref_key: PianoKey::from_midi_number(ref_note),
        ref_pitch: Pitch::from_hz(ref_pitch),
        root_offset: i32::from(origin) - i32::from(ref_note),
    };
    let builder = Kbm::builder(kbm_root).range(range);

    let (line_number, line) = next(&mut lines, KbmStructuralError::ExpectingFormalOctave)??;
    let formal_octave: i16 = parse(line_number, &line, KbmParseErrorKind::IntValue)?;
    let mut builder = builder.formal_octave(formal_octave);

    while let Some((line_number, line)) = lines.next().transpose()? {
        let main_item = main_item(&line);
        if main_item.eq_ignore_ascii_case("x") {
            builder = builder.push_unmapped_key();
        } else {
            let scale_degree = parse(line_number, main_item, KbmParseErrorKind::KeyboardMapping)?;
            builder = builder.push_mapped_key(scale_degree);
        }
    }

    for _ in builder.key_mapping.len()..usize::from(num_items) {
        builder = builder.push_unmapped_key();
    }

    let kbm = builder.build()?;
    if kbm.table.num_items() == num_items {
        Ok(kbm)
    } else {
        Err(KbmStructuralError::InconsistentNumberOfItems.into())
    }
}

/// Error reported when importing a [`Kbm`] fails.
#[derive(Debug)]
pub enum KbmImportError {
    IoError(io::Error),
    ParseError {
        line_number: usize,
        kind: KbmParseErrorKind,
    },
    StructuralError(KbmStructuralError),
    BuildError(KbmBuildError),
}

/// Specifies which kind of item is suspected to be malformed.
#[derive(Clone, Debug)]
pub enum KbmParseErrorKind {
    /// Invalid integer value or out of range.
    IntValue,

    /// Invalid float value.
    FloatValue,

    /// Invalid keyboard mapping entry. Should be "x", "X" or an integer value.
    KeyboardMapping,
}

/// Indicates that the structure of the imported [`Kbm`] file is incomplete.
#[derive(Clone, Debug)]
pub enum KbmStructuralError {
    ExpectingMapSize,
    ExpectingFirstMidiNote,
    ExpectingLastMidiNote,
    ExpectingOrigin,
    ExpectingReferenceNote,
    ExpectingReferencePitch,
    ExpectingFormalOctave,
    InconsistentNumberOfItems,
}

impl From<io::Error> for KbmImportError {
    fn from(v: io::Error) -> Self {
        Self::IoError(v)
    }
}

impl From<ParseError<KbmParseErrorKind>> for KbmImportError {
    fn from(ParseError(line_number, kind): ParseError<KbmParseErrorKind>) -> Self {
        Self::ParseError { line_number, kind }
    }
}

impl From<KbmStructuralError> for KbmImportError {
    fn from(v: KbmStructuralError) -> Self {
        Self::StructuralError(v)
    }
}

impl From<KbmBuildError> for KbmImportError {
    fn from(v: KbmBuildError) -> Self {
        Self::BuildError(v)
    }
}

fn lines(reader: impl Read) -> impl Iterator<Item = Result<(usize, String), io::Error>> {
    BufReader::new(reader)
        .lines()
        .enumerate()
        .filter_map(|(line_number, line)| match line {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('!') {
                    None
                } else {
                    Some(Ok((line_number + 1, trimmed.to_owned())))
                }
            }
            Err(err) => Some(Err(err)),
        })
}

fn next<E>(
    lines: &mut impl Iterator<Item = Result<(usize, String), io::Error>>,
    missing: E,
) -> Result<Result<(usize, String), E>, io::Error> {
    Ok(lines.next().transpose()?.ok_or(missing))
}

struct ParseError<E>(usize, E);

fn parse<T: FromStr, E>(line_number: usize, line: &str, error: E) -> Result<T, ParseError<E>> {
    main_item(line)
        .parse()
        .map_err(|_| ParseError(line_number, error))
}

fn main_item(line: &str) -> &str {
    line.split_ascii_whitespace().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scl_parse_error() {
        assert!(matches!(
            Scl::import(&b"Bad number of notes\n3x\n100.0\n5/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 2,
                kind: SclParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Scl::import(&b"Bad cents value\n3\n100.0x\n5/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 3,
                kind: SclParseErrorKind::CentsValue
            })
        ));
        assert!(matches!(
            Scl::import(&b"Bad numer\n3\n100.0\n5x/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                kind: SclParseErrorKind::Numer
            })
        ));
        assert!(matches!(
            Scl::import(&b"Bad denom\n3\n100.0\n5/4x\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                kind: SclParseErrorKind::Denom
            })
        ));
        assert!(matches!(
            Scl::import(&b"Two slashes\n3\n100.0\n5/4/3\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                kind: SclParseErrorKind::Denom
            })
        ));
        assert!(matches!(
            Scl::import(&b"Denom is empty\n3\n100.0\n5/\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                kind: SclParseErrorKind::Denom
            })
        ));
        assert!(matches!(
            Scl::import(&b"Bad integer\n3\n100.0\n5/4\n2x"[..]),
            Err(SclImportError::ParseError {
                line_number: 5,
                kind: SclParseErrorKind::IntValue
            })
        ));
    }

    #[test]
    fn scl_structural_error() {
        assert!(matches!(
            Scl::import(&b""[..]),
            Err(SclImportError::StructuralError(
                SclStructuralError::ExpectingDescription
            ))
        ));
        assert!(matches!(
            Scl::import(&b"Number of notes missing"[..]),
            Err(SclImportError::StructuralError(
                SclStructuralError::ExpectingNumberOfNotes
            ))
        ));
        assert!(matches!(
            Scl::import(&b"Bad number of notes\n7\n100.0\n5/4\n2"[..]),
            Err(SclImportError::StructuralError(
                SclStructuralError::InconsistentNumberOfNotes
            ))
        ));
        assert!(matches!(
            Scl::import(&b"Empty line\n3\n100.0\n\n2"[..]),
            Err(SclImportError::StructuralError(
                SclStructuralError::InconsistentNumberOfNotes
            ))
        ));
        assert!(Scl::import(&b"Empty line\n3\n100.0\n200.0\n2"[..]).is_ok());
    }

    #[test]
    fn kbm_parse_error() {
        assert!(matches!(
            Kbm::import(&b"Bad map size\n10\n99\n62\n69\n432\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 1,
                kind: KbmParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Kbm::import(&b"6\nBad first MIDI note\n99\n62\n69\n432\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 2,
                kind: KbmParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\nBad last MIDI note\n62\n69\n432\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 3,
                kind: KbmParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\nBad origin\n69\n432\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 4,
                kind: KbmParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\nBad reference note\n432\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 5,
                kind: KbmParseErrorKind::IntValue
            })
        ));

        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69\nBad reference pitch\n17"[..]),
            Err(KbmImportError::ParseError {
                line_number: 6,
                kind: KbmParseErrorKind::FloatValue
            })
        ));

        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69\n432\nBad formal octave"[..]),
            Err(KbmImportError::ParseError {
                line_number: 7,
                kind: KbmParseErrorKind::IntValue
            })
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69\n432\n17\nBad mapping entry"[..]),
            Err(KbmImportError::ParseError {
                line_number: 8,
                kind: KbmParseErrorKind::KeyboardMapping
            })
        ));
    }

    #[test]
    fn kbm_structural_error() {
        assert!(matches!(
            Kbm::import(&b""[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingMapSize
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingFirstMidiNote
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingLastMidiNote
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingOrigin
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingReferenceNote
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingReferencePitch
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69\n432"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::ExpectingFormalOctave
            ))
        ));
        assert!(matches!(
            Kbm::import(&b"6\n10\n99\n62\n69\n432\n17\n1\n2\n3\n4\n5\n6\n7"[..]),
            Err(KbmImportError::StructuralError(
                KbmStructuralError::InconsistentNumberOfItems
            ))
        ));
        assert!(Kbm::import(&b"6\n10\n99\n62\n69\n432\n17\n1\n2\n3\n4\n5\n6"[..]).is_ok());
    }
}
