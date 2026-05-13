use tinyharness_lib::config::load_settings;

use crate::style::*;

pub fn execute(arg: Option<&str>) {
    let settings = load_settings();

    match arg {
        Some(sub) if sub.to_lowercase() == "all" => execute_all(&settings),
        Some(other) => {
            println!(
                "{}Unknown argument '{}'.{} Use {}/settings{} to show settings or {}/settings all{} to list all safe commands.",
                ORANGE, other, RESET, BOLD, RESET, BOLD, RESET
            );
        }
        None => execute_summary(&settings),
    }
}

fn execute_summary(settings: &tinyharness_lib::config::Settings) {
    println!();
    println!(
        "{}╭─ Settings ─────────────────────────────────╮{}",
        BOLD, RESET
    );

    let provider_str = format!("{}", settings.last_provider);
    println!(
        "{}│{} Provider:  {}{}{}",
        BOLD, RESET, BLUE, provider_str, RESET
    );

    match &settings.last_model {
        Some(model) => println!("{}│{} Model:     {}{}{}", BOLD, RESET, BLUE, model, RESET),
        None => println!("{}│{} Model:     {}none{}", BOLD, RESET, ORANGE, RESET),
    }

    println!(
        "{}│{} Mode:      {}{}{}",
        BOLD, RESET, BLUE, settings.preferred_mode, RESET
    );

    match &settings.ollama_api_key {
        Some(key) => {
            let masked = if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "****".to_string()
            };
            println!("{}│{} API Key:   {}{}{}", BOLD, RESET, BLUE, masked, RESET);
        }
        None => println!("{}│{} API Key:   {}not set{}", BOLD, RESET, ORANGE, RESET),
    }

    println!(
        "{}│{} Timeout:   {}{}s{}",
        BOLD, RESET, BLUE, settings.ollama_timeout_secs, RESET
    );
    println!(
        "{}│{} Retries:   {}{}{}",
        BOLD, RESET, BLUE, settings.ollama_max_retries, RESET
    );

    match settings.context_limit {
        Some(limit) => println!(
            "{}│{} Ctx Limit: {}{} tokens{}",
            BOLD, RESET, BLUE, limit, RESET
        ),
        None => println!(
            "{}│{} Ctx Limit: {}auto (model default){}",
            BOLD, RESET, GRAY, RESET
        ),
    }

    let auto_accept_str = if settings.auto_accept_safe_commands {
        "enabled"
    } else {
        "disabled"
    };
    let auto_accept_color = if settings.auto_accept_safe_commands {
        GREEN
    } else {
        ORANGE
    };
    println!(
        "{}│{} Auto-Accept: {}{}{} (safe commands)",
        BOLD, RESET, auto_accept_color, auto_accept_str, RESET
    );

    let safe_commands = settings.get_safe_commands();
    let denied_commands = settings.get_denied_commands();
    println!(
        "{}│{} Safe Cmds:  {}{}{} configured (use {}/settings all{} to list all)",
        BOLD,
        RESET,
        BLUE,
        safe_commands.len(),
        RESET,
        BOLD,
        RESET
    );
    if !denied_commands.is_empty() {
        println!(
            "{}│{} Denied Cmds: {}{}{} always denied",
            BOLD,
            RED,
            denied_commands.len(),
            RESET,
            RESET
        );
    }

    println!(
        "{}╰────────────────────────────────────────────╯{}",
        BOLD, RESET
    );
    println!();
}

fn execute_all(settings: &tinyharness_lib::config::Settings) {
    let safe_commands = settings.get_safe_commands();
    let denied_commands = settings.get_denied_commands();

    println!();
    println!(
        "{}╭─ All Safe Commands ({} configured) ───────────╮{}",
        BOLD,
        safe_commands.len(),
        RESET
    );

    // Display 3 commands per row
    let cmds: Vec<&str> = safe_commands.iter().map(|s| s.as_str()).collect();
    let chunks = cmds.chunks(3);

    for row in chunks {
        let mut line = String::new();
        for (i, cmd) in row.iter().enumerate() {
            if i > 0 {
                // Pad between columns — 20 chars wide per column
                let prev = row[i - 1];
                let padding = 20_usize.saturating_sub(prev.len());
                line.push_str(&" ".repeat(padding));
            }
            line.push_str(&format!("{}{}{}", CYAN, cmd, RESET));
        }
        println!("{}│{}   {}", BOLD, RESET, line);
    }

    println!(
        "{}╰───────────────────────────────────────────────╯{}",
        BOLD, RESET
    );

    if !denied_commands.is_empty() {
        println!();
        println!(
            "{}╭─ Always-Deny Commands ({} configured) ────────╮{}",
            BOLD,
            denied_commands.len(),
            RESET
        );
        for cmd in &denied_commands {
            println!("{}│{}   {}✕ {} {}{}", BOLD, RESET, RED, cmd, RESET, RESET);
        }
        println!(
            "{}╰───────────────────────────────────────────────╯{}",
            BOLD, RESET
        );
    }

    println!();
}
