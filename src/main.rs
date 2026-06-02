pub mod agent;
pub mod commands;

use std::{
    error::Error,
    io::Write,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};

use tinyharness_lib::{
    config::{ProviderKind, Settings, ensure_prompts_initialized, load_settings, save_settings},
    context::WorkspaceContext,
    mode::AgentMode,
    provider::{
        Message, Provider, Role, llama_cpp::LlamaCppProvider, ollama::OllamaProvider,
        vllm::VllmProvider,
    },
    session::{Session, SessionStore},
    tools::ToolManager,
};

use crate::agent::setup as agent_setup;
use crate::{agent::run_agent_loop, commands::CommandContext};
use clap::Parser;
use tinyharness_ui::output::Output;
use tinyharness_ui::style::*;
use tokio::sync::Mutex;

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(short, long)]
    ollama: bool,
    #[arg(short, long)]
    llama_cpp: bool,
    #[arg(short, long)]
    vllm: bool,
    #[arg(short, long, default_value_t = String::new())]
    url: String,
    /// Continue the most recent session in the current directory.
    #[arg(short, long)]
    r#continue: bool,
    /// Run interactive provider setup: pick a provider, enter a URL, save to
    /// settings. Exits when done.
    #[arg(long)]
    config: bool,
    /// Start the conversation with this prompt instead of waiting for input.
    /// Use `-p` for short flags. The agent then drops into the normal
    /// interactive loop for follow-up turns.
    #[arg(short = 'p', long = "prompt")]
    prompt: Option<String>,
}

/// Determine the provider kind from CLI flags or saved settings.
fn resolve_provider_kind(args: &Args, settings: &Settings) -> ProviderKind {
    if args.llama_cpp {
        ProviderKind::LlamaCpp
    } else if args.vllm {
        ProviderKind::Vllm
    } else if args.ollama {
        ProviderKind::Ollama
    } else {
        settings.last_provider
    }
}

/// Create the provider backend, run health checks, and return it wrapped in Arc<Mutex>.
async fn create_provider(
    kind: ProviderKind,
    url: String,
    settings: &Settings,
) -> Arc<Mutex<dyn Provider + Send + Sync>> {
    let provider: Arc<Mutex<dyn Provider + Send + Sync>> = match kind {
        ProviderKind::LlamaCpp => Arc::new(Mutex::new(LlamaCppProvider::new(url))),
        ProviderKind::Vllm => Arc::new(Mutex::new(VllmProvider::new(url))),
        ProviderKind::Ollama => Arc::new(Mutex::new(OllamaProvider::new(
            url,
            settings.ollama_timeout_secs,
            settings.ollama_max_retries,
            settings.ollama_think_type,
        ))),
    };

    // Run health check for all providers (Ollama included)
    {
        let p = provider.lock().await;
        if let Err(e) = p.health_check().await {
            let mut err_out = Output::stderr();
            let _ = writeln!(
                err_out,
                "{BOLD}Error:{RESET} {kind} health check failed: {e}",
            );
            std::process::exit(1);
        }
    }

    provider
}

/// Auto-select a model on the provider if none is currently set.
/// Tries the saved model first, then falls back to the first available model.
async fn auto_select_model(provider: &mut dyn Provider, saved_model: Option<&String>) {
    if provider.current_model().is_some() {
        return;
    }

    let models = provider.list_models().await;

    if let Some(saved) = saved_model {
        if models.iter().any(|m| m == saved) {
            provider.select_model(saved.clone());
            return;
        }
        // Saved model not available — warn and fall through
        if let Some(first) = models.first() {
            let mut err_out = Output::stderr();
            let _ = writeln!(
                err_out,
                "{BOLD}Warning:{RESET} Saved model '{saved}' not available. Picked first: {BLUE}{first}{RESET}",
            );
            provider.select_model(first.clone());
        } else {
            let mut err_out = Output::stderr();
            let _ = writeln!(
                err_out,
                "{BOLD}Error:{RESET} No models available. Use /model <name> to set one manually.",
            );
        }
        return;
    }

    // No saved model — pick first available
    if let Some(first) = models.first() {
        let mut err_out = Output::stderr();
        let _ = writeln!(
            err_out,
            "{BOLD}Warning:{RESET} No model selected. Automatically picked first available model: {BLUE}{first}{RESET}",
        );
        provider.select_model(first.clone());
    } else {
        let mut err_out = Output::stderr();
        let _ = writeln!(
            err_out,
            "{BOLD}Error:{RESET} No models available. Use /model <name> to set one manually.",
        );
    }
}

/// Create a brand-new session with an initial system prompt message.
fn create_initial_session(
    working_dir: &str,
    initial_mode: AgentMode,
    provider_str: &str,
    current_model: Option<String>,
    workspace_ctx: &WorkspaceContext,
    prompts_dir: &std::path::Path,
) -> (Session, Vec<Message>) {
    let sess =
        SessionStore::default_path().create(working_dir, initial_mode, provider_str, current_model);
    let system_prompt = format!(
        "{}\n\n---\n{}",
        initial_mode.load_system_prompt(prompts_dir),
        workspace_ctx.format()
    );
    let msgs = vec![Message {
        role: Role::System,
        content: system_prompt,
        tool_calls: vec![],
        images: vec![],
    }];
    (sess, msgs)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing: library code uses tracing::warn!/error! instead of
    // direct eprintln!, so diagnostics are routed through the subscriber.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    // Install Ctrl+C handler: set an atomic flag that the agent loop checks
    // during streaming generation. This allows interrupting LLM responses
    // without terminating the process.
    let interrupted = Arc::new(AtomicBool::new(false));
    // First Ctrl+C sets a flag that the agent loop checks during streaming.
    // A second Ctrl+C exits the process immediately so the user is never stuck.
    ctrlc::set_handler({
        let interrupted = Arc::clone(&interrupted);
        move || {
            if interrupted.load(Ordering::SeqCst) {
                // Already interrupted — user wants out now
                std::process::exit(130);
            }
            interrupted.store(true, Ordering::SeqCst);
        }
    })?;

    let args = Args::parse();

    // -- Handle --config: interactive provider setup, then exit ───────────────
    if args.config {
        let mut out = Output::stdout();
        let result = agent_setup::interactive_setup(&mut out);
        match result {
            Ok(_) => return Ok(()),
            Err(e) => {
                let mut err_out = Output::stderr();
                let _ = writeln!(err_out, "{BOLD}Error:{RESET} {e}");
                std::process::exit(1);
            }
        }
    }

    // Load saved settings (will be used as defaults when no CLI flags are given)
    let settings = load_settings();

    // Determine which provider to use: CLI flags override saved settings
    let provider_kind = resolve_provider_kind(&args, &settings);

    // Resolve URL: CLI > saved > default. If a provider flag was passed without
    // --url, prompt interactively (requires a TTY) and persist the result.
    let url = if args.url.is_empty() {
        let cli_provider_flag_set = args.ollama || args.llama_cpp || args.vllm;
        if cli_provider_flag_set {
            // User explicitly chose a provider without a URL — ask for it
            // interactively so the saved URL stays in sync.
            let default = agent_setup::default_url_for(provider_kind);
            let mut out = Output::stdout();
            let url = match agent_setup::prompt_for_url(&mut out, provider_kind, default) {
                Ok(u) => u,
                Err(e) => {
                    let mut err_out = Output::stderr();
                    let _ = writeln!(err_out, "{BOLD}Error:{RESET} {e}");
                    std::process::exit(1);
                }
            };
            agent_setup::save_provider_settings(provider_kind, &url);
            url
        } else {
            agent_setup::resolve_url(provider_kind, &args.url, &settings)
        }
    } else {
        args.url.clone()
    };

    let provider = create_provider(provider_kind, url.clone(), &settings).await;

    // Auto-select model if none is currently set
    {
        let mut p = provider.lock().await;
        auto_select_model(&mut *p, settings.last_model.as_ref()).await;
    }

    // Save the provider kind + URL now that we know which one is active.
    // We persist whenever anything was explicitly chosen via CLI (provider
    // flag or --url) so the next run doesn't have to re-prompt. The
    // URL-resolution block above already calls `save_provider_settings` when
    // a provider flag was passed without --url, so we only need to cover
    // the remaining cases here.
    let mut settings = settings;
    let explicit_provider = args.ollama || args.llama_cpp || args.vllm;
    if explicit_provider && !args.url.is_empty() {
        // User gave both --ollama/--llama-cpp/--vllm and --url. Persist both.
        if settings.last_provider != provider_kind {
            settings.last_provider = provider_kind;
        }
        settings.last_provider_url = Some(url.clone());
        save_settings(&settings);
    } else if !explicit_provider && !args.url.is_empty() {
        // User gave --url only (provider resolved from saved settings).
        settings.last_provider_url = Some(url.clone());
        save_settings(&settings);
    }
    // else: no CLI override — leave settings as they are.

    // ── Collect workspace context ──────────────────────────────────────────
    let workspace_ctx = WorkspaceContext::collect();

    // ── Ensure prompt files exist (seeded from hardcoded defaults on first launch)
    let prompts_dir = ensure_prompts_initialized();

    let mut tool_manager = ToolManager::new();
    tool_manager.register_defaults();

    let initial_mode = settings.preferred_mode;

    let provider_str = provider_kind.to_string();

    // Resolve the current model name
    let current_model = {
        let p = provider.lock().await;
        p.current_model()
    };

    // ── Session persistence ───────────────────────────────────────────────
    let working_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let (mut session, mut messages) = if args.r#continue {
        let store = SessionStore::default_path();
        match store.find_latest_for_dir(&working_dir) {
            Some(session_id) => match store.load(&session_id) {
                Ok((sess, loaded_msgs)) => {
                    let meta = sess.meta();
                    let name = meta.name.as_deref().unwrap_or("unnamed");
                    let mut err_out = Output::stderr();
                    let _ = writeln!(
                        err_out,
                        "{BOLD}Resumed session {BLUE}{}{RESET} — {BOLD}{name}{RESET} ({} messages, {})",
                        &meta.id[..12],
                        meta.message_count,
                        meta.mode,
                    );
                    Some((sess, loaded_msgs))
                }
                Err(e) => {
                    let mut err_out = Output::stderr();
                    let _ = writeln!(
                        err_out,
                        "{BOLD}Warning:{RESET} Failed to resume session: {e}. Starting fresh.",
                    );
                    None
                }
            },
            None => {
                let mut err_out = Output::stderr();
                let _ = writeln!(
                    err_out,
                    "{ORANGE}No previous session found in this directory. Starting fresh.{RESET}",
                );
                None
            }
        }
    } else {
        None
    }
    .unwrap_or_else(|| {
        create_initial_session(
            &working_dir,
            initial_mode,
            &provider_str,
            current_model.clone(),
            &workspace_ctx,
            &prompts_dir,
        )
    });

    let mut ctx = CommandContext::new(Arc::clone(&provider), workspace_ctx, prompts_dir);
    ctx.current_mode = initial_mode;
    ctx.session_id = Some(session.id().to_string());

    run_agent_loop(
        provider,
        tool_manager,
        &mut messages,
        &mut ctx,
        &mut session,
        &interrupted,
        args.prompt.as_deref(),
    )
    .await
}
