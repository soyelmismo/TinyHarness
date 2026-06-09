// ── Shared Streaming Response Handling ──────────────────────────────────────
//
// Both CLI and TUI loops share identical logic for consuming the streaming
// response from the LLM provider: accumulating content, tracking tool calls,
// handling thinking/reasoning, and detecting completion/errors/interrupts.
//
// This module extracts that shared state and logic.

use std::sync::atomic::{AtomicBool, Ordering};

use tinyharness_lib::provider::{ChatMessageResponse, TokenUsage};

/// Accumulated state from consuming a streaming response.
///
/// After the streaming loop completes, the caller uses this to decide
/// what to do next (push messages, handle tool calls, retry, etc.).
#[derive(Debug)]
pub struct StreamingResult {
    /// All content text accumulated from the response.
    pub content: String,
    /// Tool calls requested by the assistant (if any).
    pub tool_calls: Vec<tinyharness_lib::provider::ToolCall>,
    /// Token usage reported by the provider (if any).
    pub token_usage: Option<TokenUsage>,
    /// Whether the provider sent a `done` event.
    pub received_done: bool,
    /// Whether the response was an error.
    pub is_error: bool,
    /// Whether the user interrupted the response (Ctrl+C).
    pub was_interrupted: bool,
    /// Accumulated thinking/reasoning content (if thinking is enabled).
    pub thinking_content: String,
}

/// Tracking state for the thinking content header.
///
/// Both loops need to know whether they've shown the "[thinking]" header
/// already, so they only print it once.
#[derive(Debug, Default)]
pub struct ThinkingState {
    /// Whether the "[thinking]" header has been shown.
    pub header_shown: bool,
    /// Accumulated thinking content.
    pub content: String,
}

/// Process a single streaming chunk.
///
/// Updates the accumulator state and returns `true` if the stream is done.
/// The caller is responsible for output rendering (CLI: stdout, TUI: channel).
///
/// Returns `Some(ProcessEvent)` describing what happened, or `None` if the
/// chunk should be ignored.
#[derive(Debug)]
pub enum ProcessEvent {
    /// New content text arrived.
    Content(String),
    /// New thinking/reasoning text arrived.
    Thinking(String),
    /// Stream is done (completed or errored).
    Done,
}

/// Accumulator for streaming response state.
#[derive(Debug, Default)]
pub struct StreamingAccumulator {
    pub content: String,
    pub tool_calls: Vec<tinyharness_lib::provider::ToolCall>,
    pub token_usage: Option<TokenUsage>,
    pub received_done: bool,
    pub is_error: bool,
}

impl StreamingAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a streaming response message.
    ///
    /// Returns a list of `ProcessEvent` describing what the caller should render.
    /// The caller should check `self.received_done` after processing to know
    /// if the stream is complete.
    pub fn process_message(&mut self, msg: &ChatMessageResponse) -> Vec<ProcessEvent> {
        let mut events = Vec::new();

        if !msg.message.tool_calls.is_empty() {
            self.tool_calls = msg.message.tool_calls.clone();
        }

        if msg.done {
            self.received_done = true;
            if let Some(ref usage) = msg.usage {
                self.token_usage = Some(usage.clone());
            }
        }

        if msg.is_error {
            self.is_error = true;
        }

        // Thinking content
        if let Some(ref thinking) = msg.message.thinking
            && !thinking.is_empty()
        {
            events.push(ProcessEvent::Thinking(thinking.clone()));
        }

        // Regular content
        if !msg.message.content.is_empty() {
            self.content.push_str(&msg.message.content);
            events.push(ProcessEvent::Content(msg.message.content.clone()));
        }

        if self.received_done {
            events.push(ProcessEvent::Done);
        }

        events
    }

    /// Check if the user has interrupted the stream.
    pub fn is_interrupted(&self, interrupted: &AtomicBool) -> bool {
        interrupted.load(Ordering::SeqCst)
    }

    /// Build the final `StreamingResult`.
    pub fn into_result(self, was_interrupted: bool) -> StreamingResult {
        StreamingResult {
            content: self.content,
            tool_calls: self.tool_calls,
            token_usage: self.token_usage,
            received_done: self.received_done,
            is_error: self.is_error,
            was_interrupted,
            thinking_content: String::new(), // caller tracks this separately
        }
    }
}
