# TinyHarness

Lightweight AI assistant framework in Rust with pluggable LLM providers (Ollama, llama.cpp, vLLM) and built-in tool calling.

## Commands

- Build: `cargo build`
- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace -- -D warnings`
- Format check: `cargo fmt --all -- --check`
- Formating: `cargo fmt --all`
- Install: `make install` (builds release + copies to `~/.local/bin`)
- Run: `cargo run` (Ollama default) or `cargo run -- --llama-cpp` / `--vllm`

## Workspace Structure

The project uses a Cargo workspace with two crates:

- **`tinyharness-lib`** — Core library crate (frontend-agnostic, no terminal I/O)
- **`tinyharness`** — Binary CLI crate (depends on `tinyharness-lib`)

### Library crate (`tinyharness-lib/`)

```
tinyharness-lib/src/
├── lib.rs               Re-exports all public types
├── provider/             Provider trait + implementations (ollama, llama_cpp, vllm, openai_compat)
├── config/mod.rs         Settings persistence (provider, model, mode, API key, denied commands)
├── mode.rs               AgentMode enum (casual/planning/agent/research) with system prompts
├── context.rs            WorkspaceContext — auto-detected project metadata + TINYHARNESS.md loading
├── session.rs            JSONL session persistence with UUIDs
├── token.rs              Token estimation and context window calculations
└── tools/                Tool implementations (ls, read, write, edit, grep, run, glob, web_search, etc.)
```

### Binary crate (`src/`)

```
src/
├── main.rs               Entry point, CLI parsing, provider creation
├── agent.rs              Main interaction loop, tool call dispatch, confirmation UI, context load warning
├── style.rs              ANSI color constants
├── commands/             Slash command handlers
│   ├── mod.rs            CommandDispatcher — parse and dispatch /commands
│   ├── command.rs        /command — manage safe/denied commands
│   ├── compact.rs        /compact — cascading summarization for long sessions
│   ├── settings.rs       /settings — configuration display (supports `all` variant)
│   └── ...               Other commands (session, model, apikey, etc.)
└── ui/                   Terminal UI helpers (confirmation prompts, input, diffs)
```

## Code Conventions

- Rust edition 2024
- Core logic lives in `tinyharness-lib` — no terminal I/O, no ANSI codes, no rustyline
- CLI-specific code (terminal output, interactive prompts) stays in the binary crate
- Binary crate imports from `tinyharness_lib` for provider, config, tools, session, etc.
- Tools registered in `tinyharness-lib/src/tools/mod.rs` via `ToolManager::register_defaults()`
- Tool definitions live in `tinyharness-lib/src/tools/<name>.rs` — each exposes a `*_tool_entry()` function returning a `Tool`
- Providers implement the `Provider` trait in `tinyharness-lib/src/provider/mod.rs`
- Settings persisted as JSON in `~/.config/tinyharness/settings.json`
- Sessions stored as JSONL in `~/.local/share/tinyharness/sessions/`
- Use `serde` + `schemars` for serialization and tool schema generation
- Minimize dependencies — prefer `std` and lightweight crates over heavy ones; avoid adding new deps when the same can be achieved with what's already in the workspace
- Prefer manual `Pin<Box<dyn Future>>` over `async-trait` to keep the dependency tree small
- Error handling: `Result<T, String>` for user-facing errors, `Result<T, Box<dyn Error>>` for internal

## Architecture

Key flow: `main.rs` → `create_provider()` → `run_agent_loop()` (in `agent.rs`) → streams responses from provider → dispatches tool calls → confirms with user for sensitive tools (write/edit/run/switch_mode) → appends results.

## Agent Modes

| Mode | Tools | Purpose |
|------|-------|---------|
| casual | None | Pure chat, no filesystem access |
| planning | read-only (ls, read, grep, glob, web_search) + switch_mode, question | Analyze & plan, then escalate to agent |
| agent | All tools | Full development access |
| research | read-only + web_search, web_fetch + switch_mode, question | Web research, then escalate |

## Testing

- Framework: built-in `#[test]` + `cargo test --workspace`
- Use `tempfile` crate in dev-dependencies for test isolation — tool tests must not write to real filesystem
- Run specific test: `cargo test <test_name>`
- Library tests: `cargo test -p tinyharness-lib`
- Binary tests: `cargo test -p TinyHarness`

## Important Rules

- Never modify `src/style.rs` ANSI codes without checking terminal compatibility
- `switch_mode` and `question` tools are handled specially in `agent.rs` — they bypass the generic tool execution path
- Confirmation for `run` tool cannot be auto-accepted even with 'a' (auto-accept) — only write/edit can
- System prompt is refreshed after mode switches, file pinning (/add, /drop), and /refresh
- Session auto-saves every 5 messages
- When adding new modules to `tinyharness-lib`, update `lib.rs` re-exports
- Command safety: `is_safe_command()` in `src/agent.rs` checks prefixes and deny list; shell redirections (`2>&1`, `>/dev/null`) are stripped before matching
- Context management: `/compact` uses cascading for sessions >60% of context window; load-time warnings at 70%/90% thresholds

## Known Gotchas

- All providers now run a health check on startup; Ollama is included
- If the saved model is unavailable, auto-select picks the first available model with a warning
- `rustyline` history stored in `~/.local/share/tinyharness/history.txt`
- Web search requires an Ollama API key set via `/apikey`
- `#[macro_export]` macros (`define_tool!`, `extract_args!`, `register_tools!`) are exported at the crate root of `tinyharness_lib`, not in the `tools` module
- Shell commands with redirections (`2>&1`, `>/dev/null`) are auto-accepted if the base command is safe — redirections are stripped before prefix matching
- Cascading compaction may produce less coherent summaries than single-pass (trade-off for handling very long sessions)
- Context load warnings are estimates based on token counting; actual usage may vary by model
- Ctrl+C interrupts the current LLM generation; a second Ctrl+C exits the process immediately

## Verification Steps

After making changes, run:
1. `cargo fmt --all` — ensure formatting is clean
2. `cargo clippy --workspace -- -D warnings` — no clippy warnings
3. `cargo test --workspace` — all tests pass
4. `cargo build` — clean release build succeeds