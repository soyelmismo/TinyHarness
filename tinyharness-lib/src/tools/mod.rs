pub mod auto_compact;
pub mod edit;
pub mod git;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod question;
pub mod read;
pub mod run;
pub mod switch_mode;
pub mod tool;
pub mod web_search;
pub mod write;

use crate::mode::AgentMode;
use crate::provider::ToolDefinition;
use crate::tools::tool::{Tool, ToolCategory};

/// Events emitted by signal-category tools that the caller must interpret.
/// These tools return a result string, but the caller should parse these
/// into structured events for proper handling (e.g., prompting the user,
/// switching mode, triggering compaction).
#[derive(Debug, Clone)]
pub enum SignalEvent {
    /// The model requests a mode switch.
    SwitchMode { mode: AgentMode },
    /// The model asks the user a question with options.
    Question {
        question: String,
        answers: Vec<String>,
    },
    /// The model requests conversation compaction.
    AutoCompact { focus: String },
}

/// Register multiple tools at once.
///
/// # Example
/// ```ignore
/// register_tools!(self,
///     crate::tools::ls::ls_tool_entry,
///     crate::tools::read::read_tool_entry,
/// );
/// ```
#[macro_export]
macro_rules! register_tools {
    ($manager:expr, $($entry:path),* $(,)?) => {
        $(
            $manager.register_tool($entry());
        )*
    };
}

#[derive(Default)]
pub struct ToolManager {
    tools: Vec<Tool>,
}

impl ToolManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register all built-in tools.
    pub fn register_defaults(&mut self) {
        register_tools!(
            self,
            crate::tools::auto_compact::auto_compact_tool_entry,
            crate::tools::ls::ls_tool_entry,
            crate::tools::read::read_tool_entry,
            crate::tools::write::write_tool_entry,
            crate::tools::edit::edit_tool_entry,
            crate::tools::grep::grep_tool_entry,
            crate::tools::run::run_tool_entry,
            crate::tools::glob::glob_tool_entry,
            crate::tools::git::git_status_tool_entry,
            crate::tools::git::git_diff_tool_entry,
            crate::tools::web_search::web_search_tool_entry,
            crate::tools::web_search::web_fetch_tool_entry,
            crate::tools::switch_mode::switch_mode_tool_entry,
            crate::tools::question::question_tool_entry,
        );
    }

    pub fn register_tool(&mut self, tool: Tool) {
        self.tools.push(tool);
    }

    /// Returns the tool definitions for all registered tools.
    pub fn get_all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.tool_info.clone()).collect()
    }

    /// Returns the tool definitions appropriate for the given agent mode.
    pub fn tools_for_mode(&self, mode: AgentMode) -> Vec<ToolDefinition> {
        match mode {
            AgentMode::Agent => self.get_all_tool_definitions(),
            AgentMode::Casual => Vec::new(),
            AgentMode::Planning => self
                .tools
                .iter()
                .filter(|t| {
                    t.category == ToolCategory::ReadOnly || t.category == ToolCategory::Signal
                })
                .map(|t| t.tool_info.clone())
                .collect(),
            AgentMode::Research => self
                .tools
                .iter()
                .filter(|t| {
                    t.category == ToolCategory::ReadOnly || t.category == ToolCategory::Signal
                })
                .map(|t| t.tool_info.clone())
                .collect(),
        }
    }

    /// Returns the category of a tool by name, or `None` if not found.
    pub fn category_of(&self, tool_name: &str) -> Option<ToolCategory> {
        self.tools
            .iter()
            .find(|t| t.name() == tool_name)
            .map(|t| t.category)
    }

    /// Returns `true` if the tool requires user approval before execution.
    /// Destructive tools (write, edit, run) and signal tools (switch_mode,
    /// question, auto_compact) require approval.
    pub fn needs_approval(&self, tool_name: &str) -> bool {
        self.category_of(tool_name)
            .map(|c| c == ToolCategory::Destructive || c == ToolCategory::Signal)
            .unwrap_or(false)
    }

    /// Returns only read-only tools (ls, read, grep, glob) — no write/edit/run.
    /// Also includes signal tools so planning/research modes can escalate.
    #[deprecated(note = "Use tools_for_mode() instead")]
    pub fn get_readonly_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|t| t.category == ToolCategory::ReadOnly || t.category == ToolCategory::Signal)
            .map(|t| t.tool_info.clone())
            .collect()
    }

    /// Returns research tools (read-only + web search/fetch, no write/edit/run).
    /// Also includes signal tools so research mode can escalate.
    #[deprecated(note = "Use tools_for_mode() instead")]
    pub fn get_research_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|t| t.category == ToolCategory::ReadOnly || t.category == ToolCategory::Signal)
            .map(|t| t.tool_info.clone())
            .collect()
    }

    /// Returns all tool definitions.
    #[deprecated(note = "Use get_all_tool_definitions() instead")]
    pub fn get_ollama_tools(&self) -> Vec<ToolDefinition> {
        self.get_all_tool_definitions()
    }

    pub async fn execute_tool_call(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> String {
        if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
            tool::execute_tool_call(tool, arguments).await
        } else {
            format!("Error: Tool '{}' not found", tool_name)
        }
    }
}
