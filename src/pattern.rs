//! Utilities for matching and updating the expected with the actual command output.
//!
//! Both the expected and actual outputs are given as slices of lines.
//! The matching works as follows (everything modulo trailing whitespaces):
//! - All normal lines, that is every line which is not "..." or "???", are matched exactly.
//! - Lines only consisting of "..." matches any number of arbitrary lines.
//! - Lines only consisting of "???" matches any number of arbitrary lines and updates the expected
//!     lines with the actual lines.

use crate::LinesCow;
use std::fmt;

#[derive(thiserror::Error, Debug)]
pub struct ParseError<'a> {
    /// The expected line or end of input.
    expected: Option<&'a str>,
    /// Got a line or end of input.
    got: Option<&'a str>,
}

impl<'a> fmt::Display for ParseError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (self.expected, self.got) {
            (Some(expected), Some(got)) => write!(f, "Expected: {expected}\nGot: {got}"),
            (Some(expected), None) => write!(f, "Expected: {expected}\nGot end of input."),
            (None, Some(got)) => write!(f, "Expected end of input\nGot: {got}"),
            _ => unreachable!(),
        }
    }
}

/// The result when parsing. The ok value is a tuple of the remaining lines and an option which is
/// `None` if nothing should be updated or `Some(lines)` if the input should be updated.
type ParseResult<'a> = Result<(&'a [&'a str], Option<Vec<&'a str>>), ParseError<'a>>;

/// Match a list of lines exactly.
/// Match exactly line by line.
fn match_lines<'a>(expected: &[&'a str], actual: &'a [&'a str]) -> ParseResult<'a> {
    let mut i = 0usize;
    while i < expected.len() {
        if i == actual.len() {
            return Err(ParseError {
                expected: Some(expected[i]),
                got: None,
            });
        }
        if expected[i].trim_end() != actual[i].trim_end() {
            return Err(ParseError {
                expected: Some(expected[i]),
                got: Some(actual[i]),
            });
        }
        i += 1;
    }
    Ok((&actual[i..], None))
}

fn with_holes<'a, const UPDATE: bool>(
    pattern: &mut impl FnMut(&[&'a str], &'a [&'a str]) -> ParseResult<'a>,
    expected: &[&'a str],
    actual: &'a [&'a str],
) -> ParseResult<'a> {
    let hole = if UPDATE { "???" } else { "..." };
    match expected.iter().position(|line| line.trim() == hole) {
        None => pattern(expected, actual),
        Some(hole_idx) => {
            let before_hole = &expected[..hole_idx];
            let after_hole = &expected[hole_idx + 1..];

            let (actual, updated_before) = pattern(before_hole, actual)?;

            let mut err = None;
            for i in 0..=actual.len() {
                match with_holes::<UPDATE>(pattern, after_hole, &actual[i..]) {
                    Err(e) => err = Some(e),
                    Ok((remaining_input, updated_after)) => {
                        let push_hole_content = |x: &mut Vec<&'a str>| {
                            if UPDATE {
                                x.extend_from_slice(&actual[..i])
                            } else {
                                x.push(expected[hole_idx])
                            }
                        };
                        let updated = match (updated_before, updated_after) {
                            (Some(mut x), Some(y)) => {
                                push_hole_content(&mut x);
                                x.extend_from_slice(&y);
                                Some(x)
                            }
                            (Some(mut x), None) => {
                                push_hole_content(&mut x);
                                x.extend_from_slice(after_hole);
                                Some(x)
                            }
                            (None, Some(y)) => {
                                let mut x = before_hole.to_vec();
                                push_hole_content(&mut x);
                                x.extend_from_slice(&y);
                                Some(x)
                            }
                            (None, None) => None,
                        };
                        return Ok((remaining_input, updated));
                    }
                }
            }
            Err(err.unwrap())
        }
    }
}

pub fn matchit<'a>(
    expected: &[&'a str],
    actual: &'a [&'a str],
) -> Result<Option<Vec<&'a str>>, ParseError<'a>> {
    let (remaining_input, updated) = with_holes::<true>(
        &mut |x, y| with_holes::<false>(&mut match_lines, x, y),
        expected,
        actual,
    )?;
    if !remaining_input.is_empty() {
        return Err(ParseError {
            expected: None,
            got: Some(remaining_input[0]),
        });
    }

    Ok(updated)
}
