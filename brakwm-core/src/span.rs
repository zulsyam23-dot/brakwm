use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// A location in the source: 1-based line/column plus 0-based byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceLoc {
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}

impl SourceLoc {
    pub const fn new(line: u32, column: u32, offset: usize) -> Self {
        Self { line, column, offset }
    }
}

impl Default for SourceLoc {
    fn default() -> Self {
        Self { line: 0, column: 0, offset: 0 }
    }
}

/// A span of source: a half-open byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: SourceLoc,
    pub end: SourceLoc,
}

impl Span {
    pub const fn new(start: SourceLoc, end: SourceLoc) -> Self {
        Self { start, end }
    }
}

impl Default for Span {
    fn default() -> Self {
        DUMMY_SPAN
    }
}

pub const DUMMY_SPAN: Span = Span {
    start: SourceLoc::new(0, 0, 0),
    end: SourceLoc::new(0, 0, 0),
};

/// Maps a filename to its source text and computes line offsets lazily.
#[derive(Debug, Clone)]
pub struct SourceMap {
    pub filename: Rc<str>,
    source: Rc<str>,
    starts: Rc<[usize]>,
}

impl SourceMap {
    pub fn new(filename: &str, source: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        let starts = starts.into_boxed_slice();

        Self {
            filename: filename.into(),
            source: Rc::from(source.to_owned()),
            starts: Rc::from(starts),
        }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Byte offset -> (line, column), both 1-based.
    pub fn locate(&self, offset: usize) -> SourceLoc {
        let offset = offset.min(self.source.len());
        let line_idx = match self.starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let line_start = self.starts[line_idx];
        let column = offset - line_start + 1;
        SourceLoc::new(line_idx as u32 + 1, column as u32, offset)
    }

    pub fn span(&self, start_off: usize, end_off: usize) -> Span {
        Span::new(self.locate(start_off), self.locate(end_off))
    }

    /// Returns the line text containing the given offset (without newline).
    pub fn line_text(&self, offset: usize) -> String {
        let line_idx = match self.starts.binary_search(&offset.min(self.source.len())) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let start = self.starts[line_idx];
        let end = self
            .starts
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.source.len());
        self.source[start..end].trim_end_matches(['\r', '\n']).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_lines() {
        let sm = SourceMap::new("t.brk", "ab\ncd\n");
        assert_eq!(sm.locate(0), SourceLoc::new(1, 1, 0));
        assert_eq!(sm.locate(3), SourceLoc::new(2, 1, 3));
        assert_eq!(sm.locate(4), SourceLoc::new(2, 2, 4));
        // Out of range clamps to EOF.
        assert_eq!(sm.locate(99), SourceLoc::new(3, 1, 6));
    }

    #[test]
    fn line_text_works() {
        let sm = SourceMap::new("t.brk", "hello\nworld");
        assert_eq!(sm.line_text(0), "hello");
        assert_eq!(sm.line_text(6), "world");
    }
}