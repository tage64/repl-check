mod pattern;
use pandoc_ast::{Block, Pandoc};
use regex::Regex;
use std::collections::hash_map::HashMap;

const TIMEOUT_MS: u64 = 3000;

#[derive(Debug)]
struct Pattern {
    components: Vec<Vec<String>>,
}

impl Pattern {
    fn new(text: &str) -> Self {
        Self {
            components: text
                .split("...")
                .map(|x| x.split("???").map(|x| x.to_string()).collect())
                .collect(),
        }
    }
}

struct Session {
    pty: rexpect::session::PtySession,
    prompt: Regex,
    expected_output_lines: Vec<String>,
}

pub fn filter(pandoc: &mut Pandoc) -> anyhow::Result<()> {
    let mut sessions = HashMap::new();
    for ref mut block in pandoc.blocks.iter_mut() {
        if let Block::CodeBlock((_, classes, attrs), ref mut code) = block {
            let Some(session_name) = classes
                    .iter()
                    .filter(|x| x.starts_with("repl-"))
                    .map(|x| &x[5..])
                    .next() else {
                        continue;
                    };
            let shell_command = attrs
                .iter()
                .filter(|(x, _)| x == "cmd")
                .map(|(_, y)| y)
                .next();
            let prompt = attrs
                .iter()
                .filter(|(x, _)| x == "prompt")
                .map(|(_, y)| y)
                .map(|x| Regex::new(x).map_err(|e| anyhow::anyhow!("In session {session_name}: Bad regular expression for prompt: {x}: {e}")))
                .next().transpose()?;
            if !sessions.contains_key(session_name) {
                let Some(shell_command) = shell_command else {
                    anyhow::bail!("No command provided at beginning of session {session_name}.");
                };
                let Some(prompt) = prompt else {
                    anyhow::bail!("Prompt must be specified for the session {session_name}.");
                };
                let pty = rexpect::spawn(shell_command.as_str(), Some(TIMEOUT_MS))?;
                sessions.insert(
                    session_name.to_string(),
                    Session {
                        pty,
                        prompt,
                        expected_output_lines: vec![],
                    },
                );
            } else {
                if let Some(shell_command) = shell_command {
                    anyhow::bail!("cmd is specified a second time for session {session_name} as `{shell_command}`.");
                }
                if let Some(prompt) = prompt {
                    sessions.get_mut(session_name).unwrap().prompt = prompt;
                }
            }
            let session = sessions.get_mut(session_name).unwrap();
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
        }
    }
    Ok(())
}
