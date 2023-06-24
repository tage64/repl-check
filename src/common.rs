/// A Vec of strings, (usually lines), which is either borrowed (`Vec<&str>`) or owned
/// (`Vec<String>`).
///
/// The idea is that we want to push strings, usually borrowed strings, but sometimes some line
/// mighg be updated, and then we want to handle the logic of changing every line from `&str` to
/// `String`.
#[derive(Debug)]
pub enum LinesCow<'a> {
    Borrowed(Vec<&'a str>),
    Owned(Vec<String>),
}

impl<'a> LinesCow<'a> {
    /// Create a new, empty list of lines.
    pub fn new() -> Self {
        LinesCow::Borrowed(Vec::new())
    }

    /// Push a slice of borrowed lines. If `self` is owned, the pushed lines will be cloned.
    pub fn push_borrowed(&mut self, lines: &[&'a str]) {
        match self {
            LinesCow::Borrowed(x) => x.extend_from_slice(lines),
            LinesCow::Owned(x) => x.extend(lines.into_iter().map(|x| x.to_string())),
        }
    }

    /// Push a slice of owned strings. If `self` is borrowed, all lines in `self` will be cloned.
    pub fn push_owned(&mut self, lines: &[&str]) {
        match self {
            LinesCow::Borrowed(x) => {
                let new_lines = x.into_iter().map(|x| x.to_string()).collect();
                *self = LinesCow::Owned(new_lines);
                self.push_owned(lines);
            }
            LinesCow::Owned(x) => x.extend(lines.into_iter().map(|x| x.to_string())),
        }
    }

    /// Convert this to `Some(lines)` if this is owned or `None` otherwise.
    pub fn maybe_owned(self) -> Option<Vec<String>> {
        match self {
            LinesCow::Owned(x) => Some(x),
            LinesCow::Borrowed(_) => None,
        }
    }
}
