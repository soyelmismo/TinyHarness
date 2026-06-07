// ── Sidebar widget ──────────────────────────────────────────────────────────
//
// Displays project context, directory structure, and active skills in a
// right-side panel. The structure section is scrollable when it overflows.

use crate::tui::cell::{Cell, Color, Style};
use crate::tui::event::{Event, Key, KeyEvent};
use crate::tui::layout::Rect;
use crate::tui::screen::Screen;
use crate::tui::widget::{Action, Widget, styles};

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
}

impl SidebarWidget {
    pub fn new() -> Self {
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
    fn content_height(&self) -> usize {
        let mut rows = 0;

        // Project section header
        rows += 2; // header + blank line
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

        rows += 1; // blank line before structure

        // Structure section
        if !self.structure.is_empty() {
            rows += 2; // header + blank line
            rows += self.structure.len();
            rows += 1; // blank line after
        }

        // Skills section
        if !self.active_skills.is_empty() {
            rows += 2; // header + blank line
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

        // We track logical row positions and skip rows that fall before
        // the scroll offset. Each "logical row" is one screen row.
        let mut logical_row = 0usize;
        let mut screen_row = area.y + 1; // skip top border area

        let skip_rows = self.scroll_offset;

        // ── Project section ────────────────────────────────────────────
        logical_row += 1; // header row
        if logical_row > skip_rows && screen_row < area.y + area.height - 1 {
            self.draw_section_header(screen, screen_row, area.x + 2, max_width, "Project");
            screen_row += 1;
        } else {
            logical_row += 0; // header already counted
        }
        logical_row += 1; // blank line after header — we just advance
        if logical_row > skip_rows {
            // skip blank — just don't draw
        }
        // Actually: the header takes 1 row, then the blank line is implicit
        // Let's simplify: each draw_section_header returns the next row,
        // and we track skip_rows directly.

        // Reset: simpler approach — draw into a virtual buffer of rows,
        // then slice based on scroll offset.

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
        if !self.structure.is_empty() {
            items.push(SidebarItem::Header("Structure".to_string()));
            for entry in &self.structure {
                items.push(SidebarItem::Entry(entry.clone()));
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
                SidebarItem::Entry(text) => {
                    let display = format!("  {}", text);
                    let truncated = if display.len() > max_width {
                        format!("{}…", &display[..max_width.saturating_sub(1)])
                    } else {
                        display
                    };
                    screen.write_str(
                        screen_row,
                        area.x + 2,
                        &truncated,
                        Color::Ansi(252),
                        styles::SIDEBAR_BG,
                        Style::default(),
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
        if let Event::Key(key) = event {
            match key {
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

/// Items that make up the sidebar content, used for scroll-aware rendering.
enum SidebarItem {
    Header(String),
    LabeledValue {
        label: String,
        value: String,
        color: Color,
    },
    Entry(String),
    Skill(String),
    Spacer,
}

impl SidebarWidget {
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
            format!("{}…", &value[..available.saturating_sub(1)])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_new() {
        let sidebar = SidebarWidget::new();
        assert!(sidebar.visible);
        assert!(sidebar.project_name.is_empty());
        assert_eq!(sidebar.scroll_offset, 0);
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
        // Should include: header(1) + name(1) + type(1) + spacer(1) + header(1) + 2 entries + spacer(1) = 8
        assert!(height >= 7);
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
}
