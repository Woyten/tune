//! Generate tuning maps to enhance the capabilities of synthesizers with limited tuning support.

mod aot;
mod jit;
mod midi;

use std::hash::Hash;

use crate::{
    note::{Note, NoteLetter},
    pitch::Ratio,
};

pub use self::{aot::*, jit::*, midi::*};

/// A note-based multichannel synthesizer with note detuning capabilities.
pub trait TunableSynth {
    type Result: IsErr;
    type NoteAttr: Clone + Default;
    type GlobalAttr;

    fn num_channels(&self) -> usize;

    fn group_by(&self) -> GroupBy;

    fn notes_detune(&mut self, channel: usize, detuned_notes: &[(Note, Ratio)]) -> Self::Result;

    fn note_on(&mut self, channel: usize, started_note: Note, attr: Self::NoteAttr)
        -> Self::Result;

    fn note_off(
        &mut self,
        channel: usize,
        stopped_note: Note,
        attr: Self::NoteAttr,
    ) -> Self::Result;

    fn note_attr(
        &mut self,
        channel: usize,
        affected_note: Note,
        attr: Self::NoteAttr,
    ) -> Self::Result;

    fn global_attr(&mut self, attr: Self::GlobalAttr) -> Self::Result;
}

pub trait IsErr {
    fn is_err(&self) -> bool;

    fn ok() -> Self;
}

impl<T: Default, E> IsErr for Result<T, E> {
    fn is_err(&self) -> bool {
        self.is_err()
    }

    fn ok() -> Self {
        Ok(T::default())
    }
}

impl IsErr for () {
    fn is_err(&self) -> bool {
        false
    }

    fn ok() -> Self {}
}

/// Defines the group that is affected by a tuning change.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GroupBy {
    /// Tuning changes are applied per [`Note`].
    ///
    /// Example: C4 and C5 are different [`Note`]s which means they can be detuned independently within a single channel.
    Note,
    /// Tuning changes are applied per [`NoteLetter`].
    ///
    /// Example: C4 and C5 share the same [`NoteLetter`] which means they cannot be detuned independently within a single channel.
    /// In order to detune them independently, at least two channels are required.
    NoteLetter,
    /// Tuning changes always affect the whole channel.
    ///
    /// For *n* keys, at least *n* channels are required.
    Channel,
}

impl GroupBy {
    fn group(self, note: Note) -> Group {
        match self {
            GroupBy::Note => Group::Note(note),
            GroupBy::NoteLetter => Group::NoteLetter(note.letter_and_octave().0),
            GroupBy::Channel => Group::Channel,
        }
    }
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
enum Group {
    Note(Note),
    NoteLetter(NoteLetter),
    Channel,
}
