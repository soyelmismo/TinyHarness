// ── Sidebar widget ──────────────────────────────────────────────────────────
//
// Displays project context, directory structure, and active skills in a
// right-side panel. The structure section is scrollable when it overflows.
// When the sidebar is in "structure mode" (Ctrl+P), the structure section
// becomes an interactive file browser where you can navigate directories.

use std::fs;
use std::path::PathBuf;

use crate::tui::cell::{Cell, Color, Style};
use crate::tui::event::{Event, Key, KeyEvent};
use crate::tui::layout::Rect;
use crate::tui::screen::Screen;
use crate::tui::widget::{Action, Widget, styles, truncate_str};

// ── Directory entry for the file browser ──────────────────────────────────────

/// A single entry in the file browser listing.
#[derive(Clone, Debug)]
struct DirEntry {
    /// Display name (just the filename, not the full path).
    name: String,
    /// Whether this entry is a directory.
    is_dir: bool,
}

impl DirEntry {
    fn icon(&self) -> &'static str {
        if self.is_dir {
            "📁"
        } else {
            match self.name.rsplit('.').next() {
                Some("rs") => "🦀",
                Some("toml") => "⚙",
                Some("md") => "📝",
                Some("json") => "{ }",
                Some("yaml" | "yml") => "📋",
                Some("py") => "🐍",
                Some("js" | "ts") => "📜",
                Some("txt") => "📄",
                Some("lock") => "🔒",
                Some("cfg" | "ini" | "conf") => "🔧",
                Some("sh" | "bash") => "🐚",
                Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp") => "🖼",
                Some("gitignore" | "env") => "🙈",
                _ => "  ",
            }
        }
    }
}

/// Read directory entries from a path, sorted (directories first, then files).
fn read_dir_sorted(path: &PathBuf) -> Vec<DirEntry> {
    let mut entries: Vec<DirEntry> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files/dirs (starting with .)
            if !name.starts_with('.') {
                entries.push(DirEntry { name, is_dir });
            }
        }
    }
    // Sort: directories first, then files; each group sorted alphabetically
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}

/// Return the icon for a file/dir entry based on its name and type.
/// Reuses the same icon logic as `DirEntry::icon()` but works with just name + is_dir.
fn icon_for_entry(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        "📁"
    } else {
        match name.rsplit('.').next() {
            Some("rs") => "🦀",
            Some("toml") => "⚙",
            Some("md") => "📝",
            Some("json") => "{ }",
            Some("yaml" | "yml") => "📋",
            Some("py") => "🐍",
            Some("js" | "ts") => "📜",
            Some("txt") => "📄",
            Some("lock") => "🔒",
            Some("cfg" | "ini" | "conf") => "🔧",
            Some("sh" | "bash") => "🐚",
            Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp") => "🖼",
            Some("gitignore" | "env") => "🙈",
            _ => "  ",
        }
    }
}

// ── Sidebar widget ───────────────────────────────────────────────────────────

/// The sidebar widget showing project context.
pub struct SidebarWidget {
    pub project_name: String,
    pub project_type: String,
    pub git_branch: Option<String>,
    pub build_command: String,
    pub test_command: String,
    /// Project directory structure (top-level listing with contents).
    pub structure: Vec<String>,
    pub active_skills: Vec<(String, String)>, // (name, description)
    pub visible: bool,
    /// Vertical scroll offset in rows (0 = top).
    scroll_offset: usize,

    // ── Interactive file browser state ─────────────────────────────────────
    /// Whether the sidebar is in interactive structure mode.
    structure_mode: bool,
    /// Current directory being browsed.
    structure_cwd: PathBuf,
    /// Navigation stack for going back (push on enter, pop on escape).
    structure_nav_stack: Vec<(PathBuf, usize)>,
    /// Directory entries in the current listing.
    structure_entries: Vec<DirEntry>,
    /// Currently selected entry index.
    structure_selected: usize,
    /// Scroll offset for the structure entries (in entry rows).
    structure_scroll: usize,
    /// The workspace root path (used to initialize the browser).
    workspace_root: PathBuf,
}

impl SidebarWidget {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            project_name: String::new(),
            project_type: String::new(),
            git_branch: None,
            build_command: String::new(),
            test_command: String::new(),
            structure: Vec::new(),
            active_skills: Vec::new(),
            visible: true,
            scroll_offset: 0,
            structure_mode: false,
            structure_cwd: cwd.clone(),
            structure_nav_stack: Vec::new(),
            structure_entries: Vec::new(),
            structure_selected: 0,
            structure_scroll: 0,
            workspace_root: cwd,
        }
    }

    /// Enter interactive structure mode (called when Focus::Structure is set).
    pub fn enter_structure_mode(&mut self) {
        if !self.structure_mode {
            self.structure_mode = true;
            self.structure_cwd = self.workspace_root.clone();
            self.structure_nav_stack.clear();
            self.structure_selected = 0;
            self.structure_scroll = 0;
            self.refresh_structure_listing();
        }
    }

    /// Exit interactive structure mode (called when focus leaves Structure).
    pub fn exit_structure_mode(&mut self) {
        self.structure_mode = false;
    }

    /// Whether the sidebar is currently in interactive structure mode.
    pub fn is_structure_mode(&self) -> bool {
        self.structure_mode
    }

    /// Set the workspace root path for the file browser.
    pub fn set_workspace_root(&mut self, path: PathBuf) {
        self.workspace_root = path;
    }

    /// Refresh the directory listing from `structure_cwd`.
    fn refresh_structure_listing(&mut self) {
        self.structure_entries = read_dir_sorted(&self.structure_cwd);
        // Clamp selection
        if !self.structure_entries.is_empty() {
            self.structure_selected = self
                .structure_selected
                .min(self.structure_entries.len() - 1);
        } else {
            self.structure_selected = 0;
        }
        self.clamp_structure_scroll();
    }

    /// Clamp structure_scroll so the selected item is visible.
    fn clamp_structure_scroll(&mut self) {
        // We'll adjust this during render based on visible rows.
        // For now, just ensure scroll <= selected.
        if self.structure_scroll > self.structure_selected {
            self.structure_scroll = self.structure_selected;
        }
    }

    /// Scroll up by `n` rows.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by `n` rows.
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
    }

    /// Scroll to the top.
    pub fn scroll_home(&mut self) {
        self.scroll_offset = 0;
    }

    /// Calculate how many visual rows the sidebar content needs
    /// (excluding the top/bottom padding rows).
    ///
    /// This must match exactly the number of `SidebarItem`s pushed in `render()`.
    fn content_height(&self) -> usize {
        let mut rows = 0;

        // Project section header
        rows += 1; // header
        if !self.project_name.is_empty() {
            rows += 1;
        }
        if !self.project_type.is_empty() {
            rows += 1;
        }
        if self.git_branch.is_some() {
            rows += 1;
        }
        if !self.build_command.is_empty() {
            rows += 1;
        }
        if !self.test_command.is_empty() {
            rows += 1;
        }

        rows += 1; // spacer before structure

        // Structure section
        if self.structure_mode {
            rows += 1; // header
            rows += 1; // breadcrumb path line
            rows += 1; // spacer after breadcrumb
            rows += self.structure_entries.len().max(1); // entries (at least 1 for "empty" msg)
            rows += 1; // spacer after entries
        } else if !self.structure.is_empty() {
            rows += 1; // header
            rows += self.structure.len();
            rows += 1; // spacer after entries
        }

        // Skills section
        if !self.active_skills.is_empty() {
            rows += 1; // header
            rows += self.active_skills.len();
        }

        rows
    }
}

impl Widget for SidebarWidget {
    fn render(&mut self, area: Rect, screen: &mut Screen) {
        if !self.visible || area.is_empty() {
            return;
        }

        // Fill background
        screen.fill_rect(
            area,
            Cell {
                char: ' ',
                fg: styles::SIDEBAR_FG,
                bg: styles::SIDEBAR_BG,
                style: Style::default(),
            },
        );

        // Draw left border
        screen.vline(
            area.x,
            area.y,
            area.y + area.height - 1,
            '│',
            styles::SIDEBAR_BORDER,
            styles::SIDEBAR_BG,
        );

        let max_width = (area.width as usize).saturating_sub(4); // account for border + padding
        let visible_rows = area.height as usize;
        let total_content = self.content_height();

        // Clamp scroll offset
        let max_scroll = total_content.saturating_sub(visible_rows);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let mut screen_row = area.y + 1; // skip top border area
        let skip_rows = self.scroll_offset;

        // Build the list of drawable items
        let mut items: Vec<SidebarItem> = Vec::new();

        // Project header
        items.push(SidebarItem::Header("Project".to_string()));
        if !self.project_name.is_empty() {
            items.push(SidebarItem::LabeledValue {
                label: "Name:".to_string(),
                value: self.project_name.clone(),
                color: Color::WHITE,
            });
        }
        if !self.project_type.is_empty() {
            items.push(SidebarItem::LabeledValue {
                label: "Type:".to_string(),
                value: self.project_type.clone(),
                color: Color::Ansi(14),
            });
        }
        if let Some(ref branch) = self.git_branch {
            items.push(SidebarItem::LabeledValue {
                label: "Git:".to_string(),
                value: branch.clone(),
                color: Color::GREEN,
            });
        }
        if !self.build_command.is_empty() {
            items.push(SidebarItem::LabeledValue {
                label: "Build:".to_string(),
                value: self.build_command.clone(),
                color: Color::Ansi(252),
            });
        }
        if !self.test_command.is_empty() {
            items.push(SidebarItem::LabeledValue {
                label: "Test:".to_string(),
                value: self.test_command.clone(),
                color: Color::Ansi(252),
            });
        }
        items.push(SidebarItem::Spacer);

        // Structure section
        if self.structure_mode {
            items.push(SidebarItem::Header("Structure".to_string()));
            // Breadcrumb path
            let path_display = self.format_breadcrumb(max_width);
            items.push(SidebarItem::Breadcrumb(path_display));
            items.push(SidebarItem::Spacer);

            if self.structure_entries.is_empty() {
                items.push(SidebarItem::StructureEntry {
                    icon: "  ".to_string(),
                    name: "(empty)".to_string(),
                    is_dir: false,
                    selected: false,
                });
            } else {
                for (i, entry) in self.structure_entries.iter().enumerate() {
                    items.push(SidebarItem::StructureEntry {
                        icon: entry.icon().to_string(),
                        name: entry.name.clone(),
                        is_dir: entry.is_dir,
                        selected: i == self.structure_selected,
                    });
                }
            }
            items.push(SidebarItem::Spacer);
        } else if !self.structure.is_empty() {
            items.push(SidebarItem::Header("Structure".to_string()));
            // Parse and sort structure entries: dirs first, then files (matching interactive mode)
            let mut parsed: Vec<(String, String, bool)> = self
                .structure
                .iter()
                .map(|entry| {
                    let is_dir = entry.ends_with('/') || entry.contains("/  (");
                    let name = if is_dir {
                        // "dirname/  (children)" → "dirname"
                        entry.split('/').next().unwrap_or(entry).trim().to_string()
                    } else {
                        entry.clone()
                    };
                    let icon = icon_for_entry(&name, is_dir).to_string();
                    (icon, name, is_dir)
                })
                .collect();
            parsed.sort_by(|a, b| match (a.2, b.2) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
            });
            for (icon, name, is_dir) in parsed {
                items.push(SidebarItem::Entry { icon, name, is_dir });
            }
            items.push(SidebarItem::Spacer);
        }

        // Skills section
        if !self.active_skills.is_empty() {
            items.push(SidebarItem::Header("Skills".to_string()));
            for (name, _desc) in &self.active_skills {
                items.push(SidebarItem::Skill(name.clone()));
            }
        }

        // Render items with scroll offset
        let max_item = skip_rows;
        let mut item_idx = 0usize;
        let mut drawn_rows = 0usize;
        let available_rows = visible_rows.saturating_sub(2); // top/bottom margin

        // Track which screen rows correspond to structure entries for scroll clamping
        let mut structure_entry_screen_rows: Vec<u16> = Vec::new();

        for item in &items {
            if item_idx < max_item {
                item_idx += 1;
                continue;
            }
            if drawn_rows >= available_rows {
                break;
            }
            if screen_row >= area.y + area.height - 1 {
                break;
            }

            match item {
                SidebarItem::Header(title) => {
                    self.draw_section_header(screen, screen_row, area.x + 2, max_width, title);
                }
                SidebarItem::LabeledValue {
                    label,
                    value,
                    color,
                } => {
                    self.draw_labeled_value(
                        screen,
                        screen_row,
                        area.x + 2,
                        max_width,
                        label,
                        value,
                        *color,
                    );
                }
                SidebarItem::Entry { icon, name, is_dir } => {
                    let suffix = if *is_dir { "/" } else { "" };
                    let display = format!("{} {}{}", icon, name, suffix);
                    let truncated = if display.len() > max_width {
                        format!("{}…", truncate_str(&display, max_width.saturating_sub(1)))
                    } else {
                        display
                    };
                    let fg = if *is_dir {
                        Color::BRIGHT_CYAN
                    } else {
                        Color::Ansi(252)
                    };
                    screen.write_str(
                        screen_row,
                        area.x + 2,
                        &format!("  {}", truncated),
                        fg,
                        styles::SIDEBAR_BG,
                        Style::default(),
                    );
                }
                SidebarItem::StructureEntry {
                    icon,
                    name,
                    is_dir,
                    selected,
                } => {
                    structure_entry_screen_rows.push(screen_row);
                    let suffix = if *is_dir { "/" } else { "" };
                    let display = format!("{} {}{}", icon, name, suffix);
                    let truncated = if display.len() > max_width {
                        format!("{}…", truncate_str(&display, max_width.saturating_sub(1)))
                    } else {
                        display
                    };

                    if *selected {
                        // Highlighted row: inverted or accent background
                        let sel_bg = Color::Ansi(240); // slightly lighter than sidebar bg
                        let sel_fg = Color::WHITE;
                        // Fill the entire row with selection background
                        for col in 0..area.width.saturating_sub(2) {
                            if let Some(cell) = screen.get_mut(screen_row, area.x + 1 + col) {
                                cell.bg = sel_bg;
                            }
                        }
                        screen.write_str(
                            screen_row,
                            area.x + 2,
                            &format!("▶ {}", truncated),
                            sel_fg,
                            sel_bg,
                            Style::bold(),
                        );
                    } else {
                        let fg = if *is_dir {
                            Color::BRIGHT_CYAN
                        } else {
                            Color::Ansi(252)
                        };
                        screen.write_str(
                            screen_row,
                            area.x + 2,
                            &format!("  {}", truncated),
                            fg,
                            styles::SIDEBAR_BG,
                            Style::default(),
                        );
                    }
                }
                SidebarItem::Breadcrumb(path) => {
                    screen.write_str(
                        screen_row,
                        area.x + 2,
                        path,
                        Color::Ansi(178), // warm yellow for path
                        styles::SIDEBAR_BG,
                        Style::dim(),
                    );
                }
                SidebarItem::Skill(name) => {
                    let display = format!("⚡ {}", name);
                    screen.write_str(
                        screen_row,
                        area.x + 2,
                        &display,
                        Color::CYAN,
                        styles::SIDEBAR_BG,
                        Style::bold(),
                    );
                }
                SidebarItem::Spacer => {
                    // Just a blank row — background already filled
                }
            }

            screen_row += 1;
            drawn_rows += 1;
            item_idx += 1;
        }

        // Ensure selected structure entry is visible
        if self.structure_mode && !structure_entry_screen_rows.is_empty() {
            let sel_offset_in_entries = self.structure_selected;
            if sel_offset_in_entries < structure_entry_screen_rows.len() {
                let sel_screen_row = structure_entry_screen_rows[sel_offset_in_entries];
                let top = area.y + 1;
                let bottom = area.y + area.height - 2;
                if sel_screen_row < top || sel_screen_row > bottom {
                    // Selected item not visible — adjust scroll offset
                    let _entries_before_header = items
                        .iter()
                        .position(|it| matches!(it, SidebarItem::StructureEntry { .. }))
                        .unwrap_or(0);
                    let target_scroll = if sel_screen_row < top {
                        self.scroll_offset
                            .saturating_sub((top - sel_screen_row) as usize)
                    } else {
                        self.scroll_offset + (sel_screen_row - bottom) as usize
                    };
                    self.scroll_offset = target_scroll.min(max_scroll);
                }
            }
        }

        // Draw scrollbar if content overflows
        if total_content > available_rows {
            let scrollbar_height = available_rows;
            let thumb_size = ((scrollbar_height * scrollbar_height) / total_content).max(1) as u16;
            let thumb_position = if total_content > available_rows {
                (self.scroll_offset as u16 * (scrollbar_height as u16 - thumb_size))
                    / (total_content as u16 - available_rows as u16)
            } else {
                0
            };
            let sb_x = area.x + area.width - 1;
            let sb_top = area.y + 1;
            let sb_bottom = area.y + area.height - 1;

            // Draw scrollbar track
            for row in sb_top..sb_bottom {
                if let Some(cell) = screen.get_mut(row, sb_x) {
                    cell.char = '│';
                    cell.fg = styles::SCROLLBAR_FG;
                    cell.bg = styles::SIDEBAR_BG;
                }
            }

            // Draw thumb
            for i in 0..thumb_size {
                let row = sb_top + thumb_position + i;
                if row < sb_bottom {
                    if let Some(cell) = screen.get_mut(row, sb_x) {
                        cell.char = '█';
                        cell.fg = styles::SCROLLBAR_FG;
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> Action {
        if self.structure_mode {
            return self.handle_structure_event(event);
        }

        if let Event::Key(key) = event {
            match key {
                KeyEvent {
                    key: Key::Tab,
                    modifiers,
                } if !modifiers.shift && !modifiers.alt && !modifiers.ctrl => {
                    Action::CycleFocusForward
                }
                KeyEvent {
                    key: Key::BackTab, ..
                } => Action::CycleFocusBackward,
                KeyEvent {
                    key: Key::Up,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    self.scroll_up(1);
                    Action::None
                }
                KeyEvent {
                    key: Key::Down,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    self.scroll_down(1);
                    Action::None
                }
                KeyEvent {
                    key: Key::PageUp, ..
                } => {
                    self.scroll_up(10);
                    Action::None
                }
                KeyEvent {
                    key: Key::PageDown, ..
                } => {
                    self.scroll_down(10);
                    Action::None
                }
                KeyEvent { key: Key::Home, .. } => {
                    self.scroll_home();
                    Action::None
                }
                _ => Action::None,
            }
        } else {
            Action::None
        }
    }
}

impl SidebarWidget {
    /// Handle keyboard events in interactive structure mode.
    fn handle_structure_event(&mut self, event: &Event) -> Action {
        if let Event::Key(key) = event {
            match key {
                // Up arrow: move selection up
                KeyEvent {
                    key: Key::Up,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    if self.structure_selected > 0 {
                        self.structure_selected -= 1;
                    }
                    self.ensure_selected_visible();
                    Action::None
                }
                // Down arrow: move selection down
                KeyEvent {
                    key: Key::Down,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    if !self.structure_entries.is_empty()
                        && self.structure_selected < self.structure_entries.len() - 1
                    {
                        self.structure_selected += 1;
                    }
                    self.ensure_selected_visible();
                    Action::None
                }
                // Enter: enter directory (or do nothing for files)
                KeyEvent {
                    key: Key::Enter,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    if let Some(entry) = self.structure_entries.get(self.structure_selected) {
                        if entry.is_dir {
                            let new_path = self.structure_cwd.join(&entry.name);
                            // Save current position in nav stack
                            self.structure_nav_stack
                                .push((self.structure_cwd.clone(), self.structure_selected));
                            self.structure_cwd = new_path;
                            self.structure_selected = 0;
                            self.structure_scroll = 0;
                            self.refresh_structure_listing();
                        }
                    }
                    Action::None
                }
                // Escape: go back to parent directory, or exit structure mode at root
                KeyEvent {
                    key: Key::Escape,
                    modifiers,
                } if !modifiers.alt && !modifiers.ctrl => {
                    if let Some((prev_cwd, prev_selected)) = self.structure_nav_stack.pop() {
                        self.structure_cwd = prev_cwd;
                        self.structure_selected = prev_selected;
                        self.structure_scroll = 0;
                        self.refresh_structure_listing();
                        Action::None
                    } else {
                        // At root — exit structure mode
                        self.structure_mode = false;
                        Action::ExitStructureMode
                    }
                }
                // PageUp: scroll up in entries
                KeyEvent {
                    key: Key::PageUp, ..
                } => {
                    // Move selection up by ~10
                    let step = 10.min(self.structure_selected);
                    self.structure_selected -= step;
                    self.ensure_selected_visible();
                    Action::None
                }
                // PageDown: scroll down in entries
                KeyEvent {
                    key: Key::PageDown, ..
                } => {
                    if !self.structure_entries.is_empty() {
                        let max_sel = self.structure_entries.len() - 1;
                        self.structure_selected = (self.structure_selected + 10).min(max_sel);
                        self.ensure_selected_visible();
                    }
                    Action::None
                }
                // Home: jump to first entry
                KeyEvent { key: Key::Home, .. } => {
                    self.structure_selected = 0;
                    self.ensure_selected_visible();
                    Action::None
                }
                // End: jump to last entry
                KeyEvent { key: Key::End, .. } => {
                    if !self.structure_entries.is_empty() {
                        self.structure_selected = self.structure_entries.len() - 1;
                        self.ensure_selected_visible();
                    }
                    Action::None
                }
                _ => Action::None,
            }
        } else {
            Action::None
        }
    }

    /// Adjust scroll offset so the selected entry is visible.
    fn ensure_selected_visible(&mut self) {
        let visible_entry_rows = 12; // conservative estimate
        if self.structure_selected < self.structure_scroll {
            self.structure_scroll = self.structure_selected;
        } else if self.structure_selected >= self.structure_scroll + visible_entry_rows {
            self.structure_scroll = self.structure_selected - visible_entry_rows + 1;
        }

        // Convert structure_scroll to global scroll_offset.
        let items_before_structure = self.count_items_before_structure();
        self.scroll_offset = items_before_structure + self.structure_scroll;
        // Clamp — render will also clamp, but keeping it sane here prevents issues
        let total = self.content_height();
        let max_scroll = total.saturating_sub(1); // at least 1 item visible
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
    }

    /// Count sidebar items before the first StructureEntry.
    ///
    /// This must match exactly the items pushed in `render()` before the
    /// `StructureEntry` items appear.
    fn count_items_before_structure(&self) -> usize {
        // Header("Project")
        let mut count = 1;
        if !self.project_name.is_empty() {
            count += 1;
        }
        if !self.project_type.is_empty() {
            count += 1;
        }
        if self.git_branch.is_some() {
            count += 1;
        }
        if !self.build_command.is_empty() {
            count += 1;
        }
        if !self.test_command.is_empty() {
            count += 1;
        }
        count += 1; // spacer before structure
        if self.structure_mode {
            count += 1; // Header("Structure")
            count += 1; // Breadcrumb
            count += 1; // spacer after breadcrumb
        }
        count
    }

    /// Format the current directory as a breadcrumb for display.
    fn format_breadcrumb(&self, max_width: usize) -> String {
        // Show the path relative to workspace root, or just the last 2 components
        let path = &self.structure_cwd;
        let rel = path.strip_prefix(&self.workspace_root).unwrap_or(path);
        let display = rel.to_string_lossy().to_string();
        if display.is_empty() {
            return ".".to_string();
        }
        // Truncate if too long
        if display.len() > max_width {
            let prefix = "…";
            let avail = max_width.saturating_sub(prefix.len());
            if avail > 0 {
                let start = display.len().saturating_sub(avail);
                // Find char boundary
                let mut start = start;
                while start < display.len() && !display.is_char_boundary(start) {
                    start += 1;
                }
                format!("{}{}", prefix, &display[start..])
            } else {
                prefix.to_string()
            }
        } else {
            display
        }
    }

    fn draw_section_header(
        &self,
        screen: &mut Screen,
        row: u16,
        col: u16,
        max_width: usize,
        title: &str,
    ) -> u16 {
        let header = format!("┌─ {} ", title);
        screen.write_str(
            row,
            col,
            &header,
            styles::SIDEBAR_BORDER,
            styles::SIDEBAR_BG,
            Style::bold(),
        );
        // Fill remaining space with ─
        let remaining = max_width.saturating_sub(header.len());
        if remaining > 0 {
            screen.write_str(
                row,
                col + header.len() as u16,
                &"─".repeat(remaining),
                styles::SIDEBAR_BORDER,
                styles::SIDEBAR_BG,
                Style::default(),
            );
        }
        row + 1
    }

    fn draw_labeled_value(
        &self,
        screen: &mut Screen,
        row: u16,
        col: u16,
        max_width: usize,
        label: &str,
        value: &str,
        value_color: Color,
    ) -> u16 {
        screen.write_str(
            row,
            col,
            label,
            Color::Ansi(244),
            styles::SIDEBAR_BG,
            Style::dim(),
        );
        let value_col = col + label.len() as u16 + 1;
        let available = max_width.saturating_sub(label.len() + 1);
        let display = if value.len() > available {
            format!("{}…", truncate_str(value, available.saturating_sub(1)))
        } else {
            value.to_string()
        };
        screen.write_str(
            row,
            value_col,
            &display,
            value_color,
            styles::SIDEBAR_BG,
            Style::default(),
        );
        row + 1
    }
}

/// Items that make up the sidebar content, used for scroll-aware rendering.
enum SidebarItem {
    Header(String),
    LabeledValue {
        label: String,
        value: String,
        color: Color,
    },
    Entry {
        icon: String,
        name: String,
        is_dir: bool,
    },
    StructureEntry {
        icon: String,
        name: String,
        is_dir: bool,
        selected: bool,
    },
    Breadcrumb(String),
    Skill(String),
    Spacer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_new() {
        let sidebar = SidebarWidget::new();
        assert!(sidebar.visible);
        assert!(sidebar.project_name.is_empty());
        assert_eq!(sidebar.scroll_offset, 0);
        assert!(!sidebar.structure_mode);
    }

    #[test]
    fn test_sidebar_scroll() {
        let mut sidebar = SidebarWidget::new();
        sidebar.scroll_down(5);
        assert_eq!(sidebar.scroll_offset, 5);
        sidebar.scroll_up(3);
        assert_eq!(sidebar.scroll_offset, 2);
        sidebar.scroll_home();
        assert_eq!(sidebar.scroll_offset, 0);
    }

    #[test]
    fn test_sidebar_render() {
        let mut screen = Screen::new(80, 24);
        let mut sidebar = SidebarWidget::new();
        sidebar.project_name = "TinyHarness".to_string();
        sidebar.project_type = "Rust".to_string();
        sidebar.build_command = "cargo build".to_string();
        sidebar.structure = vec!["src/  (main.rs)".to_string(), "Cargo.toml".to_string()];

        let area = Rect::new(60, 1, 20, 22);
        sidebar.render(area, &mut screen);

        // Should have rendered content in the sidebar area
        assert!(screen.get(1, 60).unwrap().char == '│');
    }

    #[test]
    fn test_sidebar_hidden() {
        let mut screen = Screen::new(80, 24);
        let mut sidebar = SidebarWidget::new();
        sidebar.visible = false;

        let area = Rect::new(60, 1, 20, 22);
        sidebar.render(area, &mut screen);

        // Should not have rendered anything
        assert_eq!(screen.get(1, 60).unwrap().char, ' '); // default
    }

    #[test]
    fn test_sidebar_content_height() {
        let mut sidebar = SidebarWidget::new();
        sidebar.project_name = "Test".to_string();
        sidebar.project_type = "Rust".to_string();
        sidebar.structure = vec!["a".to_string(), "b".to_string()];
        let height = sidebar.content_height();
        assert!(height > 0);
        // header(1) + name(1) + type(1) + spacer(1) + header(1) + 2 entries + spacer(1) = 8
        assert_eq!(height, 8);
    }

    #[test]
    fn test_sidebar_scroll_render() {
        let mut screen = Screen::new(80, 24);
        let mut sidebar = SidebarWidget::new();
        sidebar.project_name = "Test".to_string();
        sidebar.project_type = "Rust".to_string();
        sidebar.structure = (0..50).map(|i| format!("file_{}.rs", i)).collect();

        let area = Rect::new(60, 1, 20, 22);
        sidebar.render(area, &mut screen);
        // Should render without panic even with many items

        // Scroll down and re-render
        sidebar.scroll_down(5);
        sidebar.render(area, &mut screen);
        assert_eq!(sidebar.scroll_offset, 5);
    }

    #[test]
    fn test_sidebar_structure_mode() {
        let mut sidebar = SidebarWidget::new();
        assert!(!sidebar.is_structure_mode());
        sidebar.enter_structure_mode();
        assert!(sidebar.is_structure_mode());
        sidebar.exit_structure_mode();
        assert!(!sidebar.is_structure_mode());
    }

    #[test]
    fn test_sidebar_structure_navigation() {
        let mut sidebar = SidebarWidget::new();
        sidebar.enter_structure_mode();
        // Verify entries were loaded
        // (depends on the actual filesystem at test time, so just check no panic)
        let entries = sidebar.structure_entries.len();
        // Navigate within entries
        if entries > 1 {
            sidebar.structure_selected = 0;
            // Simulate down arrow
            let event = Event::Key(KeyEvent {
                key: Key::Down,
                modifiers: crate::tui::event::Modifiers::new(),
            });
            sidebar.handle_event(&event);
            assert_eq!(sidebar.structure_selected, 1);
            // Simulate up arrow
            let event = Event::Key(KeyEvent {
                key: Key::Up,
                modifiers: crate::tui::event::Modifiers::new(),
            });
            sidebar.handle_event(&event);
            assert_eq!(sidebar.structure_selected, 0);
        }
    }

    #[test]
    fn test_read_dir_sorted() {
        // Test that read_dir_sorted doesn't panic on current directory
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let entries = read_dir_sorted(&cwd);
        // Just verify it returns something (or empty if no access)
        // Entries should be sorted: dirs first, then files
        let mut last_was_dir = true;
        for entry in &entries {
            if !last_was_dir && entry.is_dir {
                // This shouldn't happen — dirs should come first
                panic!("Directories should come before files in sorted listing");
            }
            last_was_dir = entry.is_dir;
        }
    }

    #[test]
    fn test_format_breadcrumb() {
        let sidebar = SidebarWidget::new();
        // Test with workspace root == cwd (should show ".")
        let breadcrumb = sidebar.format_breadcrumb(30);
        // At least it shouldn't panic
        assert!(!breadcrumb.is_empty() || breadcrumb == ".");
    }
}
