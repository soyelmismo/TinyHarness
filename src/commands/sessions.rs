use tinyharness_lib::session::{Session, SessionMeta, format_age};

use crate::style::*;

/// Format a session for display in the `/sessions` listing.
fn format_session_list(sessions: &[SessionMeta], current_id: Option<&str>) -> String {
    let mut output = String::new();

    if sessions.is_empty() {
        output.push_str(&format!("{}No sessions found.{}", ORANGE, RESET));
        return output;
    }

    output.push_str(&format!(
        "{}Available sessions (most recent first):{}\n\n",
        BOLD, RESET
    ));

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for meta in sessions {
        let is_current = current_id == Some(meta.id.as_str());
        let marker = if is_current {
            format!("{}▸{}", CYAN, RESET)
        } else {
            " ".to_string()
        };

        let age = format_age(now_secs.saturating_sub(meta.updated_at));
        let name_str = meta.name.as_deref().unwrap_or("unnamed");

        // Truncate working_dir for display
        let dir_display = if meta.working_dir.len() > 40 {
            let end = meta.working_dir.len().saturating_sub(37);
            let start = meta.working_dir.floor_char_boundary(end);
            format!("...{}", &meta.working_dir[start..])
        } else {
            meta.working_dir.clone()
        };

        output.push_str(&format!(
            "{} {}{}{} — {}{}{}\n",
            marker,
            BLUE,
            &meta.id[..12],
            RESET,
            BOLD,
            name_str,
            RESET,
        ));
        output.push_str(&format!(
            "  {}  {}{} msgs, {}{}{}  {}{}\n",
            marker, GRAY, meta.message_count, ITALIC, age, RESET, GRAY, dir_display,
        ));
    }

    output
}

pub fn execute_list(current_session_id: Option<&str>) {
    let sessions = Session::list_all();
    let output = format_session_list(&sessions, current_session_id);
    println!("{}", output);
}
