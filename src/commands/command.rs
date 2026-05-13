use tinyharness_lib::config::{get_default_safe_commands, load_settings, save_settings};

use crate::style::*;

pub fn execute_add(cmd: &str) {
    if cmd.is_empty() {
        println!(
            "{}Usage:{} /command add <command>{} — e.g. /command add docker",
            BOLD, RESET, RESET
        );
        return;
    }

    let mut settings = load_settings();
    let mut commands = settings.get_safe_commands();

    if commands.contains(&cmd.to_string()) {
        println!(
            "{}Command '{}' is already in the safe list.{}",
            ORANGE, cmd, RESET
        );
        return;
    }

    commands.push(cmd.to_string());
    settings.safe_command_prefixes = Some(commands);
    save_settings(&settings);
    println!(
        "{}Added '{}' to auto-accepted commands.{}",
        GREEN, cmd, RESET
    );
}

pub fn execute_remove(cmd: &str) {
    if cmd.is_empty() {
        println!(
            "{}Usage:{} /command rm <command>{} — e.g. /command rm docker",
            BOLD, RESET, RESET
        );
        return;
    }

    let mut settings = load_settings();
    let mut commands = settings.get_safe_commands();
    let initial_len = commands.len();

    commands.retain(|c| c != cmd);

    if commands.len() < initial_len {
        settings.safe_command_prefixes = if commands.is_empty() {
            None // Revert to defaults if empty
        } else {
            Some(commands)
        };
        save_settings(&settings);
        println!(
            "{}Removed '{}' from auto-accepted commands.{}",
            GREEN, cmd, RESET
        );
    } else {
        println!(
            "{}Command '{}' not found in auto-accepted list.{}",
            ORANGE, cmd, RESET
        );
    }
}

pub fn execute_deny(cmd: &str) {
    if cmd.is_empty() {
        println!(
            "{}Usage:{} /command deny <command>{} — e.g. /command deny git push",
            BOLD, RESET, RESET
        );
        return;
    }

    let mut settings = load_settings();
    let mut denied = settings.get_denied_commands();

    if denied.contains(&cmd.to_string()) {
        println!(
            "{}Command '{}' is already in the deny list.{}",
            ORANGE, cmd, RESET
        );
        return;
    }

    denied.push(cmd.to_string());
    settings.denied_command_prefixes = Some(denied);
    save_settings(&settings);
    println!(
        "{}Denied '{}' — it will always require confirmation.{}",
        RED, cmd, RESET
    );
}

pub fn execute_undeny(cmd: &str) {
    if cmd.is_empty() {
        println!(
            "{}Usage:{} /command undeny <command>{} — e.g. /command undeny git push",
            BOLD, RESET, RESET
        );
        return;
    }

    let mut settings = load_settings();
    let mut denied = settings.get_denied_commands();
    let initial_len = denied.len();

    denied.retain(|c| c != cmd);

    if denied.len() < initial_len {
        settings.denied_command_prefixes = if denied.is_empty() {
            None
        } else {
            Some(denied)
        };
        save_settings(&settings);
        println!("{}Removed '{}' from the deny list.{}", GREEN, cmd, RESET);
    } else {
        println!(
            "{}Command '{}' not found in the deny list.{}",
            ORANGE, cmd, RESET
        );
    }
}

pub fn execute_list() {
    let settings = load_settings();
    let commands = settings.get_safe_commands();
    let denied = settings.get_denied_commands();
    let using_defaults = settings.safe_command_prefixes.is_none();

    println!();
    println!(
        "{}╭─ Auto-Accepted Commands ─────────────────────╮{}",
        BOLD, RESET
    );
    if using_defaults {
        println!(
            "{}│{} {}Using defaults{} ({} commands){}",
            BOLD,
            RESET,
            GRAY,
            RESET,
            commands.len(),
            RESET
        );
    } else {
        println!(
            "{}│{} {}{}{} commands configured{}",
            BOLD,
            RESET,
            BLUE,
            commands.len(),
            RESET,
            RESET
        );
    }

    // Display 3 commands per row
    let cmds: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
    for row in cmds.chunks(3) {
        let mut line = String::new();
        for (i, cmd) in row.iter().enumerate() {
            if i > 0 {
                let prev = row[i - 1];
                let padding = 20_usize.saturating_sub(prev.len());
                line.push_str(&" ".repeat(padding));
            }
            line.push_str(&format!("{}{}{}", CYAN, cmd, RESET));
        }
        println!("{}│{}   {}", BOLD, RESET, line);
    }

    println!(
        "{}╰──────────────────────────────────────────────╯{}",
        BOLD, RESET
    );

    if !denied.is_empty() {
        println!();
        println!(
            "{}╭─ Always-Deny Commands ──────────────────────╮{}",
            BOLD, RESET
        );
        println!(
            "{}│{} {}{}{} commands denied (always require confirmation){}",
            BOLD,
            RESET,
            RED,
            denied.len(),
            RESET,
            RESET
        );
        for cmd in &denied {
            println!("{}│{}   {}✕ {} {}{}", BOLD, RESET, RED, cmd, RESET, RESET);
        }
        println!(
            "{}╰──────────────────────────────────────────────╯{}",
            BOLD, RESET
        );
    }

    println!();
}

pub fn execute_reset() {
    let mut settings = load_settings();
    let defaults = get_default_safe_commands();
    let count = defaults.len();
    settings.safe_command_prefixes = None;
    save_settings(&settings);
    println!(
        "{}Reset auto-accepted commands to defaults ({} commands).{}",
        GREEN, count, RESET
    );
}

pub fn execute_reset_deny() {
    let mut settings = load_settings();
    if settings.get_denied_commands().is_empty() {
        println!("{}Deny list is already empty.{}", ORANGE, RESET);
        return;
    }
    let count = settings.get_denied_commands().len();
    settings.denied_command_prefixes = None;
    save_settings(&settings);
    println!(
        "{}Cleared deny list (removed {} command{}).{}",
        GREEN,
        count,
        if count == 1 { "" } else { "s" },
        RESET
    );
}
