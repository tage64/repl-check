#![allow(unused)]

mod pattern;
use pandoc_ast::{Block, Pandoc};
use regex::Regex;
use std::collections::hash_map::HashMap;
use std::rc::Rc;

const TIMEOUT_MS: u64 = 3000;
const DEFAULT_PROMPT_CHAR: &str = ":";

#[derive(Debug)]
struct PandocBlock<'a> {
    session_name: &'a str,
    classes: &'a Vec<String>,
    attrs: &'a Vec<(String, String)>,
    code: &'a String,
}

fn iter_code_blocks<'a>(pandoc: &'a Pandoc) -> impl Iterator<Item = PandocBlock<'a>> + 'a {
    pandoc.blocks.iter().filter_map(|block| {
        if let pandoc_ast::Block::CodeBlock((_, classes, attrs), code) = block {
            if let Some(session_name) = classes
                .iter()
                .filter(|x| x.starts_with("repl-"))
                .map(|x| &x[5..])
                .next()
            {
                Some(PandocBlock {
                    session_name,
                    classes,
                    attrs,
                    code,
                })
            } else {
                None
            }
        } else {
            None
        }
    })
}

#[derive(Debug)]
struct ReplBlock<'a> {
    prompt: Rc<Regex>,
    prompt_char: &'a str,
    expected_lines: Vec<&'a str>,
}

#[derive(Debug)]
struct Session<'a> {
    shell_cmd: &'a str,
    blocks: Vec<ReplBlock<'a>>,
}

fn get_sessions<'a>(pandoc: &'a Pandoc) -> anyhow::Result<HashMap<&'a str, Session<'a>>> {
    let mut sessions = HashMap::new();
    for PandocBlock {
        session_name,
        classes,
        attrs,
        code,
    } in iter_code_blocks(pandoc)
    {
        let shell_cmd = attrs
            .iter()
            .filter(|(x, _)| x == "cmd")
            .map(|(_, y)| y.as_str())
            .next();
        let prompt = attrs
            .iter()
            .filter(|(x, _)| x == "prompt")
            .map(|(_, y)| y)
            .map(|x| {
                Regex::new(x).map(Rc::new).map_err(|e| {
                    anyhow::anyhow!(
                        "In session {session_name}: Bad regular expression for prompt: {x}: {e}"
                    )
                })
            })
            .next()
            .transpose()?;
        let prompt_char = attrs
            .iter()
            .filter(|(x, _)| x == "prompt_char")
            .map(|(_, y)| y.as_str())
            .next();
        let expected_lines = code.lines().collect();

        use std::collections::hash_map::Entry::*;
        match sessions.entry(session_name) {
            Vacant(entry) => {
                let Some(shell_cmd) = shell_cmd else {
                    anyhow::bail!("No command provided at beginning of session {session_name}.");
                };
                let Some(prompt) = prompt else {
                    anyhow::bail!("Prompt must be specified for the session {session_name}.");
                };
                let prompt_char = prompt_char.unwrap_or(DEFAULT_PROMPT_CHAR);
                entry.insert(Session {
                    shell_cmd,
                    blocks: vec![ReplBlock {
                        prompt,
                        prompt_char,
                        expected_lines,
                    }],
                });
            }
            Occupied(mut entry) => {
                if let Some(shell_cmd) = shell_cmd {
                    anyhow::bail!("cmd is specified a second time for session {session_name} as `{shell_cmd}`.");
                }
                let last_block = entry.get().blocks.last().unwrap();
                let prompt = prompt.unwrap_or_else(|| last_block.prompt.clone());
                let prompt_char = prompt_char.unwrap_or(last_block.prompt_char);
                entry.get_mut().blocks.push(ReplBlock {
                    prompt,
                    prompt_char,
                    expected_lines,
                });
            }
        }
    }
    Ok(sessions)
}

/*
            let mut output_buf = String::new();
            for line in code.lines() {
                if let Some(prompt_match) = session.prompt.find_at(line, 0) {
                    let (_, command) = line.split_at(prompt_match.end());
                    let (output, prompt) = session.pty.exp_regex(&("\n.*".to_string() + &line))?;
                    if !session.prompt.is_match_at(&prompt, 1) {
                        output_buf += &output;
                        output_buf += &prompt;
                        continue;
                    }
                    if output_buf.is_empty() {
                        output_buf = output;
                    } else {
                        output_buf += &output;
                    }

                    let mut output_regex = r"(?m)\A\s*?".to_string();
                    for expected_line in session.expected_output_lines.iter() {
                        if expected_line == "..." {
                            output_regex += r"(^.*$)*?";
                        } else {
                            output_regex += r"^\s*?";
                            output_regex += regex::escape(expected_line).as_str();
                            output_regex += r"\s*$";
                        }
                    }
                    output_regex += r"\s*\z";

                    if !Regex::new(&output_regex).unwrap().is_match(&output_buf) {
                        eprintln!("Output mismatch:  Expected:");
                        for x in session.expected_output_lines.iter() {
                            eprintln!("{x}");
                        }
                        eprintln!("\nBut got:");
                        eprintln!("{output_buf}");
                        anyhow::bail!("Bad output from session {session_name}");
                    }

                    session.pty.send_line(command.trim())?;
                } else {
                    session.expected_output_lines.push(line.to_string());
                }
            }
*/
