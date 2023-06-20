#![allow(unused)]
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

type ParseResult<'a> = Result<(&'a [&'a str], Option<Vec<&'a str>>), ParseError<'a>>;

fn match_lines<'a>(lines: &[&'a str], input_lines: &'a [&'a str]) -> ParseResult<'a> {
    let mut i = 0;
    while i < lines.len() {
        if i == input_lines.len() {
            return Err(ParseError {
                expected: Some(lines[i]),
                got: None,
            });
        }
        if lines[i] != input_lines[i] {
            return Err(ParseError {
                expected: Some(lines[i]),
                got: Some(input_lines[i]),
            });
        }
        i += 1;
    }
    Ok((&input_lines[i..], None))
}

fn with_holes<'a, const UPDATE: bool>(
    pattern: &mut impl FnMut(&[&'a str], &'a [&'a str]) -> ParseResult<'a>,
    lines: &[&'a str],
    input_lines: &'a [&'a str],
) -> ParseResult<'a> {
    let hole = if UPDATE { "???" } else { "..." };
    match lines.iter().position(|line| line.trim() == hole) {
        None => pattern(lines, input_lines),
        Some(hole_idx) => {
            let before_hole = &lines[..hole_idx];
            let after_hole = &lines[hole_idx + 1..];

            let (input_lines, updated_before) = pattern(before_hole, input_lines)?;

            let mut err = None;
            for i in 0..=input_lines.len() {
                match with_holes::<UPDATE>(pattern, after_hole, &input_lines[i..]) {
                    Err(e) => err = Some(e),
                    Ok((remaining_input, updated_after)) => {
                        let push_hole_content = |x: &mut Vec<&'a str>| {
                            if UPDATE {
                                x.extend_from_slice(&input_lines[..i])
                            } else {
                                x.push(lines[hole_idx])
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
    pattern_lines: &[&'a str],
    input_lines: &'a [&'a str],
) -> Result<Option<Vec<String>>, ParseError<'a>> {
    let (remaining_input, updated) = with_holes::<true>(
        &mut |x, y| with_holes::<false>(&mut match_lines, x, y),
        pattern_lines,
        input_lines,
    )?;
    if !remaining_input.is_empty() {
        return Err(ParseError {
            expected: None,
            got: Some(remaining_input[0]),
        });
    }

    Ok(updated.map(|x| x.into_iter().map(str::to_string).collect()))
}
