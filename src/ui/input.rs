use rustyline::{
    Completer, Helper, Highlighter, Hinter,
    completion::Completer,
    highlight::Highlighter,
    hint::Hinter,
    validate::{ValidationContext, ValidationResult, Validator},
};

use crate::commands::CommandDispatcher;
use crate::style::*;

#[derive(Completer, Helper, Highlighter, Hinter)]
pub struct CommandHelper {
    #[rustyline(Completer)]
    completer: CommandCompleter,
    #[rustyline(Hinter)]
    hinter: CommandHinter,
    #[rustyline(Highlighter)]
    highlighter: CommandHighlighter,
}

impl Validator for CommandHelper {
    fn validate(&self, ctx: &mut ValidationContext<'_>) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();

        // Check if input ends with backslash (continuation character)
        let trimmed = input.trim_end();
        if trimmed.ends_with('\\') {
            // Incomplete - needs more input
            return Ok(ValidationResult::Incomplete);
        }

        // Check for unclosed code fences (```)
        let fence_count = input.matches("```").count();
        if fence_count % 2 == 1 {
            // Unclosed code fence - needs more input
            return Ok(ValidationResult::Incomplete);
        }

        // Check for unclosed backtick code spans
        let backtick_count = input.matches('`').count();
        if backtick_count % 2 == 1 {
            // Unclosed backtick - needs more input
            return Ok(ValidationResult::Incomplete);
        }

        // Input is complete
        Ok(ValidationResult::Valid(None))
    }
}

impl Default for CommandHelper {
    fn default() -> Self {
        Self {
            completer: CommandCompleter,
            hinter: CommandHinter,
            highlighter: CommandHighlighter,
        }
    }
}

impl CommandHelper {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct CommandCompleter;

impl Completer for CommandCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if !line.starts_with('/') || pos == 0 {
            return Ok((0, vec![]));
        }

        let prefix = &line[..pos];
        let cmd_prefix = prefix.to_lowercase();

        let matches: Vec<String> = CommandDispatcher::command_names()
            .iter()
            .filter(|name| name.starts_with(&cmd_prefix))
            .take(3)
            .map(|s| s.to_string())
            .collect();

        if matches.is_empty() {
            return Ok((0, vec![]));
        }

        Ok((0, matches))
    }
}

pub struct CommandHinter;

impl Hinter for CommandHinter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        if !line.starts_with('/') || pos == 0 || line.len() != pos {
            return None;
        }

        let prefix = line.to_lowercase();
        let matches: Vec<&str> = CommandDispatcher::command_names()
            .iter()
            .filter(|name| name.starts_with(&prefix))
            .take(3)
            .copied()
            .collect();

        if matches.is_empty() {
            return None;
        }

        if matches.len() == 1 {
            let hint = matches[0][pos..].to_string();
            if !hint.is_empty() {
                return Some(hint);
            }
        }

        let suggestions = matches.join("  ");
        Some(format!("  ({})", suggestions))
    }
}

pub struct CommandHighlighter;

impl Highlighter for CommandHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        if line.starts_with('/') {
            std::borrow::Cow::Owned(format!("{}{}{}", TITLE_COLOR, line, RESET))
        } else {
            std::borrow::Cow::Borrowed(line)
        }
    }

    fn highlight_hint<'l>(&self, hint: &'l str) -> std::borrow::Cow<'l, str> {
        std::borrow::Cow::Owned(format!("{}{}{}", GRAY, hint, RESET))
    }

    fn highlight_candidate<'l>(
        &self,
        candidate: &'l str,
        _completion: rustyline::CompletionType,
    ) -> std::borrow::Cow<'l, str> {
        std::borrow::Cow::Owned(format!("{}{}{}", TITLE_COLOR, candidate, RESET))
    }
}
