#![allow(unused)]

mod common;
mod pattern;
use common::LinesCow;
use pandoc_ast::{Block, Pandoc};
use regex::Regex;
use std::collections::hash_map::HashMap;
use std::iter;
use std::rc::Rc;

const TIMEOUT_MS: u64 = 10000;
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

/// A parsed code block which should be verified in a REPL.
#[derive(Debug)]
struct ReplBlock<'a> {
    /// A regex matching the prompt. Both in the expected an dactual output.
    prompt: Rc<Regex>,

    /// TODO: Is this needed?
    prompt_char: &'a str,

    /// A list of the expected lines (including prompt-lines).
    expected: Vec<&'a str>,
}

/// All [ReplBlock]s belonging to the same invocation of the REPL program.
#[derive(Debug)]
struct Session<'a> {
    /// The command used to run the repl from a system shell.
    shell_cmd: &'a str,

    /// An oredered list of all [ReplBlock]s.
    blocks: Vec<ReplBlock<'a>>,
}

/// Given a pandoc document, collect all REPL sessions with their names.
fn get_sessions<'a>(document: &'a Pandoc) -> anyhow::Result<HashMap<&'a str, Session<'a>>> {
    let mut sessions = HashMap::new();
    for PandocBlock {
        session_name,
        classes,
        attrs,
        code,
    } in iter_code_blocks(document)
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
        let expected = code.lines().collect();

        use std::collections::hash_map::Entry::*;
        match sessions.entry(session_name) {
            Vacant(entry) => {
                let Some(shell_cmd) = shell_cmd else {
                    anyhow::bail!("No command provided at beginning of session {session_name}.");
                };
                let Some(prompt) = prompt else {
                    anyhow::bail!("ExpectedPrompt must be specified for the session {session_name}.");
                };
                let prompt_char = prompt_char.unwrap_or(DEFAULT_PROMPT_CHAR);
                entry.insert(Session {
                    shell_cmd,
                    blocks: vec![ReplBlock {
                        prompt,
                        prompt_char,
                        expected,
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
                    expected,
                });
            }
        }
    }
    Ok(sessions)
}

/// The kind of prompt that is expected.
#[derive(Debug)]
enum ExpectedPrompt<'a> {
    /// The prompt should match exactly this string.
    Fixed(&'a str),

    /// The prompt should match the provided prompt regex.
    Flexible,

    /// The prompt should match the provided prompt regex and the promptstring in the document
    /// should be updated with the actual prompt.
    Updatable,
}

/// Information about invoking a command in a REPL.
#[derive(Debug)]
struct CmdInvokation<'a> {
    prompt: ExpectedPrompt<'a>,

    /// The command to run.
    cmd: &'a str,

    /// The prompt and the command together as it appeared in the document.
    entire_prompt_line: &'a str,

    /// Lines of expected output.
    expected_output: &'a [&'a str],
}

/// A list of command invokations.
#[derive(Debug)]
struct CmdInvokations<'a> {
    /// The expected output (list of lines) before the first command.
    initial_output: &'a [&'a str],
    cmd_invocations: Vec<CmdInvokation<'a>>,
}

fn repl_block_to_cmd_invocations<'a>(repl_block: &ReplBlock<'a>) -> CmdInvokations<'a> {
    unimplemented!()
}

/// Run a set of [Session]s.
///
/// Returns for every session a [Vec] with one element for each [ReplBlock] in that session. An
/// element in the vector is [Some] iff that block should be updated.
fn run_sessions<'a>(
    sessions: HashMap<&'a str, Session<'a>>,
) -> anyhow::Result<HashMap<String, Vec<Option<String>>>> {
    let mut updated_blocks = HashMap::new();
    for (session_name, session) in sessions.into_iter() {
        let mut process = rexpect::spawn(session.shell_cmd, Some(TIMEOUT_MS))?;

        // A list of all updated blocks in this session.
        let mut updated_repl_blocks = Vec::new();
        for repl_block in session.blocks {
            // All the lines in this block, perhaps updated.
            let mut updated_repl_block = LinesCow::new();

            let CmdInvokations {
                mut initial_output,
                cmd_invocations,
            } = repl_block_to_cmd_invocations(&repl_block);
            // Loop through all [CmdInvokation]s. [initial_output] will be updated with the
            // expected output before the prompt.
            for CmdInvokation {
                prompt,
                cmd,
                entire_prompt_line,
                expected_output,
            } in cmd_invocations
            {
                // A regex for matching the prompt in the REPL.
                let prompt_regex = match prompt {
                    ExpectedPrompt::Fixed(x) => Regex::new(&regex::escape(x)).unwrap(),
                    ExpectedPrompt::Flexible | ExpectedPrompt::Updatable => {
                        repl_block.prompt.as_ref().clone()
                    }
                };
                let (before_prompt, actual_prompt) = process
                    .reader
                    .read_until(&rexpect::ReadUntil::Regex(prompt_regex))?;
                let read_lines: Vec<&str> = before_prompt.lines().collect();
                if let Some(updated) = pattern::matchit(initial_output, &read_lines)
                    .map_err(|e| anyhow::anyhow!("Pattern mismatch: {e}"))?
                {
                    updated_repl_block.push_owned(updated.as_slice());
                } else {
                    updated_repl_block.push_borrowed(initial_output);
                }

                match prompt {
                    ExpectedPrompt::Updatable => {
                        updated_repl_block.push_owned(&[&format!("{}{}", actual_prompt, cmd)])
                    }
                    ExpectedPrompt::Flexible | ExpectedPrompt::Fixed(_) => {
                        updated_repl_block.push_borrowed(&[entire_prompt_line])
                    }
                }
            }
            // TODO: Match the rest of the output.
            updated_repl_blocks.push(
                updated_repl_block
                    .maybe_owned()
                    .map(|x| x.into_iter().reduce(|x, y| x + "\n" + &y).unwrap()),
            );
        }
        updated_blocks.insert(session_name.to_string(), updated_repl_blocks);
    }
    Ok(updated_blocks)
}
