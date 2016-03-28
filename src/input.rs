// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::char;
use std::cmp::Ordering;
use std::fmt;
use std::ops;
use std::u32;

use syntax;

use utf8::{decode_utf8, decode_last_utf8};
use literals::LiteralSearcher;

/// Represents a location in the input.
#[derive(Clone, Copy, Debug)]
pub struct InputAt {
    pos: usize,
    c: Char,
    byte: Option<u8>,
    len: usize,
}

impl InputAt {
    /// Returns true iff this position is at the beginning of the input.
    pub fn is_start(&self) -> bool {
        self.pos == 0
    }

    /// Returns true iff this position is past the end of the input.
    pub fn is_end(&self) -> bool {
        self.c.is_none() && self.byte.is_none()
    }

    /// Returns the character at this position.
    ///
    /// If this position is just before or after the input, then an absent
    /// character is returned.
    pub fn char(&self) -> Char {
        self.c
    }

    /// Returns the byte at this position.
    pub fn byte(&self) -> Option<u8> {
        self.byte
    }

    /// Returns the UTF-8 width of the character at this position.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the byte offset of this position.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the byte offset of the next position in the input.
    pub fn next_pos(&self) -> usize {
        self.pos + self.len
    }
}

/// An abstraction over input used in the matching engines.
pub trait Input {
    /// Return an encoding of the position at byte offset `i`.
    fn at(&self, i: usize) -> InputAt;

    /// Return the Unicode character occurring next to `at`.
    ///
    /// If no such character could be decoded, then `Char` is absent.
    fn next_char(&self, at: InputAt) -> Char;

    /// Return the Unicode character occurring previous to `at`.
    ///
    /// If no such character could be decoded, then `Char` is absent.
    fn previous_char(&self, at: InputAt) -> Char;

    /// Scan the input for a matching prefix.
    fn prefix_at(
        &self,
        prefixes: &LiteralSearcher,
        at: InputAt,
    ) -> Option<InputAt>;

    /// The number of bytes in the input.
    fn len(&self) -> usize;

    /// Return the given input as a sequence of bytes.
    fn as_bytes(&self) -> &[u8];
}

impl<'a, T: Input> Input for &'a T {
    fn at(&self, i: usize) -> InputAt { (**self).at(i) }

    fn next_char(&self, at: InputAt) -> Char { (**self).next_char(at) }

    fn previous_char(&self, at: InputAt) -> Char { (**self).previous_char(at) }

    fn prefix_at(
        &self,
        prefixes: &LiteralSearcher,
        at: InputAt,
    ) -> Option<InputAt> {
        (**self).prefix_at(prefixes, at)
    }

    fn len(&self) -> usize { (**self).len() }

    fn as_bytes(&self) -> &[u8] { (**self).as_bytes() }
}

/// An input reader over characters.
#[derive(Clone, Copy, Debug)]
pub struct CharInput<'t>(&'t [u8]);

impl<'t> CharInput<'t> {
    /// Return a new character input reader for the given string.
    pub fn new(s: &'t [u8]) -> CharInput<'t> {
        CharInput(s)
    }
}

impl<'t> ops::Deref for CharInput<'t> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.0
    }
}

impl<'t> Input for CharInput<'t> {
    // This `inline(always)` increases throughput by almost 25% on the `hard`
    // benchmarks over a normal `inline` annotation.
    //
    // I'm not sure why `#[inline]` isn't enough to convince LLVM, but it is
    // used *a lot* in the guts of the matching engines.
    #[inline(always)]
    fn at(&self, i: usize) -> InputAt {
        let c = decode_utf8(&self[i..]).map(|(c, _)| c).into();
        InputAt {
            pos: i,
            c: c,
            byte: None,
            len: c.len_utf8(),
        }
    }

    fn next_char(&self, at: InputAt) -> Char {
        at.char()
    }

    fn previous_char(&self, at: InputAt) -> Char {
        decode_last_utf8(&self[..at.pos()]).map(|(c, _)| c).into()
    }

    fn prefix_at(
        &self,
        prefixes: &LiteralSearcher,
        at: InputAt,
    ) -> Option<InputAt> {
        prefixes.find(&self[at.pos()..]).map(|(s, _)| self.at(at.pos() + s))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn as_bytes(&self) -> &[u8] {
        self.0
    }
}

/// An input reader over bytes.
///
/// N.B. We represent the reader with a string for now, since that gives us
/// easy access to necessary Unicode decoding (used for word boundary look
/// ahead/look behind).
#[derive(Clone, Copy, Debug)]
pub struct ByteInput<'t>(&'t [u8]);

impl<'t> ByteInput<'t> {
    /// Return a new byte-based input reader for the given string.
    pub fn new(s: &'t [u8]) -> ByteInput<'t> {
        ByteInput(s)
    }
}

impl<'t> ops::Deref for ByteInput<'t> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.0
    }
}

impl<'t> Input for ByteInput<'t> {
    #[inline(always)]
    fn at(&self, i: usize) -> InputAt {
        InputAt {
            pos: i,
            c: None.into(),
            byte: self.get(i).map(|&b| b),
            len: 1,
        }
    }

    fn next_char(&self, at: InputAt) -> Char {
        decode_utf8(&self[at.pos()..]).map(|(c, _)| c).into()
    }

    fn previous_char(&self, at: InputAt) -> Char {
        decode_last_utf8(&self[..at.pos()]).map(|(c, _)| c).into()
    }

    fn prefix_at(
        &self,
        prefixes: &LiteralSearcher,
        at: InputAt,
    ) -> Option<InputAt> {
        prefixes.find(&self[at.pos()..]).map(|(s, _)| self.at(at.pos() + s))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn as_bytes(&self) -> &[u8] {
        self.0
    }
}

/// An inline representation of `Option<char>`.
///
/// This eliminates the need to do case analysis on `Option<char>` to determine
/// ordinality with other characters.
///
/// (The `Option<char>` is not related to encoding. Instead, it is used in the
/// matching engines to represent the beginning and ending boundaries of the
/// search text.)
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Char(u32);

impl fmt::Debug for Char {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match char::from_u32(self.0) {
            None => write!(f, "Empty"),
            Some(c) => write!(f, "{:?}", c),
        }
    }
}

impl Char {
    /// Returns true iff the character is absent.
    #[inline]
    pub fn is_none(self) -> bool { self.0 == u32::MAX }

    /// Returns the length of the character's UTF-8 encoding.
    ///
    /// If the character is absent, then `0` is returned.
    #[inline]
    pub fn len_utf8(self) -> usize {
        char::from_u32(self.0).map_or(0, |c| c.len_utf8())
    }

    /// Returns true iff the character is a word character.
    ///
    /// If the character is absent, then false is returned.
    pub fn is_word_char(self) -> bool {
        char::from_u32(self.0).map_or(false, syntax::is_word_char)
    }

    /// Returns true iff the byte is a word byte.
    ///
    /// If the byte is absent, then false is returned.
    pub fn is_word_byte(self) -> bool {
        match char::from_u32(self.0) {
            None => false,
            Some(c) if c <= '\u{FF}' => syntax::is_word_byte(c as u8),
            Some(_) => false,
        }
    }

    /// Converts the character to a real primitive `char`.
    ///
    /// If the character is absent, then `None` is returned.
    pub fn as_char(self) -> Option<char> {
        // This is only used in the `regex!` macro because it expands char
        // classes into `match` expressions (instead of binary search).
        char::from_u32(self.0)
    }
}

impl From<char> for Char {
    fn from(c: char) -> Char { Char(c as u32) }
}

impl From<Option<char>> for Char {
    fn from(c: Option<char>) -> Char {
        c.map_or(Char(u32::MAX), |c| c.into())
    }
}

impl PartialEq<char> for Char {
    #[inline]
    fn eq(&self, other: &char) -> bool { self.0 == *other as u32 }
}

impl PartialEq<Char> for char {
    #[inline]
    fn eq(&self, other: &Char) -> bool { *self as u32 == other.0 }
}

impl PartialOrd<char> for Char {
    #[inline]
    fn partial_cmp(&self, other: &char) -> Option<Ordering> {
        self.0.partial_cmp(&(*other as u32))
    }
}

impl PartialOrd<Char> for char {
    #[inline]
    fn partial_cmp(&self, other: &Char) -> Option<Ordering> {
        (*self as u32).partial_cmp(&other.0)
    }
}
