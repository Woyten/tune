use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::ops::Range;
use std::str::FromStr;

use crate::key::PianoKey;
use crate::pitch::Pitch;
use crate::scala::Kbm;
use crate::scala::KbmBuildError;
use crate::scala::KbmBuilder;
use crate::scala::KbmRoot;
use crate::scala::Scl;
use crate::scala::SclBuildError;
use crate::scala::SclBuilder;

pub(crate) fn import_scl(reader: impl Read) -> Result<Scl, SclImportError> {
    let importer = SclImporter::ExpectingDescription;
    consume_lines(importer, reader, |i, line_number, line| {
        i.consume(line_number, line)
    })
    .and_then(|i| i.finalize())
}

enum SclImporter {
    ExpectingDescription,
    ExpectingNumberOfNotes(String),
    ConsumingPitchLines(String, u16, SclBuilder),
}

impl SclImporter {
    fn consume(self, line_number: usize, line: &str) -> Result<Self, SclImportError> {
        Ok(match self {
            SclImporter::ExpectingDescription => {
                SclImporter::ExpectingNumberOfNotes(line.to_owned())
            }
            SclImporter::ExpectingNumberOfNotes(description) => {
                let num_notes = parse(line_number, line, SclParseErrorKind::IntValue)?;
                SclImporter::ConsumingPitchLines(description, num_notes, Scl::builder())
            }
            SclImporter::ConsumingPitchLines(description, num_notes, mut builder) => {
                let main_item = main_item(line);
                if main_item.contains('.') {
                    let cents_value = parse(line_number, main_item, SclParseErrorKind::CentsValue)?;
                    builder = builder.push_cents(cents_value);
                } else if let Some((numer, denom)) = main_item.split_once('/') {
                    let numer = parse(line_number, numer, SclParseErrorKind::Numer)?;
                    let denom = parse(line_number, denom, SclParseErrorKind::Denom)?;
                    builder = builder.push_fraction(numer, denom);
                } else {
                    let int_value = parse(line_number, main_item, SclParseErrorKind::IntValue)?;
                    builder = builder.push_int(int_value)
                }
                SclImporter::ConsumingPitchLines(description, num_notes, builder)
            }
        })
    }

    fn finalize(self) -> Result<Scl, SclImportError> {
        let error = match self {
            SclImporter::ExpectingDescription => SclStructuralError::ExpectingDescription,
            SclImporter::ExpectingNumberOfNotes(..) => SclStructuralError::ExpectingNumberOfNotes,
            SclImporter::ConsumingPitchLines(description, num_notes, builder) => {
                let scl = builder.build_with_description(description)?;
                if scl.num_items() == num_notes {
                    return Ok(scl);
                };
                SclStructuralError::InconsistentNumberOfNotes
            }
        };
        Err(error.into())
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
    let importer = KbmImporter::ExpectingMapSize;
    consume_lines(importer, reader, |i, line_number, line| {
        i.consume(line_number, line)
    })
    .and_then(|i| i.finalize())
}

enum KbmImporter {
    ExpectingMapSize,
    ExpectingFirstMidiNote(u16),
    ExpectingLastMidiNote(u16, PianoKey),
    ExpectingOrigin(u16, Range<PianoKey>),
    ExpectingReferenceNote(u16, Range<PianoKey>, i16),
    ExpectingReferencePitch(u16, Range<PianoKey>, i16, i16),
    ExpectingFormalOctave(u16, KbmBuilder),
    ConsumingMapLines(u16, KbmBuilder),
}

impl KbmImporter {
    fn consume(self, line_number: usize, line: &str) -> Result<Self, KbmImportError> {
        Ok(match self {
            KbmImporter::ExpectingMapSize => {
                let num_items = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ExpectingFirstMidiNote(num_items)
            }
            KbmImporter::ExpectingFirstMidiNote(num_items) => {
                let midi_number: i32 = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ExpectingLastMidiNote(
                    num_items,
                    PianoKey::from_midi_number(midi_number),
                )
            }
            KbmImporter::ExpectingLastMidiNote(num_items, range_start) => {
                let midi_number: i32 = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ExpectingOrigin(
                    num_items,
                    range_start..PianoKey::from_midi_number(midi_number).plus_steps(1),
                )
            }
            KbmImporter::ExpectingOrigin(num_items, range) => {
                let origin = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ExpectingReferenceNote(num_items, range, origin)
            }
            KbmImporter::ExpectingReferenceNote(num_items, range, origin) => {
                let ref_note = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ExpectingReferencePitch(num_items, range, origin, ref_note)
            }
            KbmImporter::ExpectingReferencePitch(num_items, range, origin, ref_note) => {
                let ref_pitch = parse(line_number, line, KbmParseErrorKind::FloatValue)?;
                let kbm_root = KbmRoot {
                    ref_key: PianoKey::from_midi_number(ref_note),
                    ref_pitch: Pitch::from_hz(ref_pitch),
                    root_offset: i32::from(origin) - i32::from(ref_note),
                };
                KbmImporter::ExpectingFormalOctave(num_items, Kbm::builder(kbm_root).range(range))
            }
            KbmImporter::ExpectingFormalOctave(num_items, builder) => {
                let formal_octave = parse(line_number, line, KbmParseErrorKind::IntValue)?;
                KbmImporter::ConsumingMapLines(num_items, builder.formal_octave(formal_octave))
            }
            KbmImporter::ConsumingMapLines(num_items, mut builder) => {
                let main_item = main_item(line);
                if ["x", "X"].contains(&main_item) {
                    builder = builder.push_unmapped_key();
                } else {
                    let scale_degree =
                        parse(line_number, main_item, KbmParseErrorKind::KeyboardMapping)?;
                    builder = builder.push_mapped_key(scale_degree);
                }
                KbmImporter::ConsumingMapLines(num_items, builder)
            }
        })
    }

    fn finalize(self) -> Result<Kbm, KbmImportError> {
        let error = match self {
            KbmImporter::ExpectingMapSize => KbmStructuralError::ExpectingMapSize,
            KbmImporter::ExpectingFirstMidiNote(..) => KbmStructuralError::ExpectingFirstMidiNote,
            KbmImporter::ExpectingLastMidiNote(..) => KbmStructuralError::ExpectingLastMidiNote,
            KbmImporter::ExpectingOrigin(..) => KbmStructuralError::ExpectingOrigin,
            KbmImporter::ExpectingReferenceNote(..) => KbmStructuralError::ExpectingReferenceNote,
            KbmImporter::ExpectingReferencePitch(..) => KbmStructuralError::ExpectingReferencePitch,
            KbmImporter::ExpectingFormalOctave(..) => KbmStructuralError::ExpectingFormalOctave,
            KbmImporter::ConsumingMapLines(num_items, mut builder) => {
                for _ in builder.key_mapping.len()..usize::from(num_items) {
                    builder = builder.push_unmapped_key();
                }
                let kbm = builder.build()?;
                if kbm.num_items() <= num_items {
                    return Ok(kbm);
                }
                KbmStructuralError::InconsistentNumberOfItems
            }
        };
        Err(error.into())
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

pub(crate) fn consume_lines<I, R: From<io::Error>>(
    mut importer: I,
    reader: impl Read,
    mut consume: impl FnMut(I, usize, &str) -> Result<I, R>,
) -> Result<I, R> {
    for (line_number, line) in BufReader::new(reader).lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('!') {
            importer = consume(importer, line_number + 1, trimmed)?;
        }
    }
    Ok(importer)
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
