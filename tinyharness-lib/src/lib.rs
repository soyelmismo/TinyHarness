pub mod config;
pub mod context;
pub mod mode;
pub mod provider;
pub mod session;
pub mod token;
pub mod tools;

// Re-export key types at crate root for convenience
pub use config::{ProviderKind, Settings, SettingsError, SettingsStore};
pub use context::WorkspaceContext;
pub use mode::AgentMode;
pub use provider::{
    ChatMessage, ChatMessageResponse, Message, Provider, Role, TokenUsage, ToolCall,
    ToolCallFunction, ToolDefinition, ToolFunctionDef, ToolKind,
};
pub use session::{Session, SessionEntry, SessionMeta, SessionStore};
pub use token::ContextWindowSize;
pub use tools::tool::ToolCategory;
pub use tools::{SignalEvent, ToolManager};

// #[macro_export] macros are automatically at the crate root:
// - define_tool!
// - extract_args!
// - register_tools!
