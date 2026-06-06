// ── Conversation widget ─────────────────────────────────────────────────────
//
// Displays the conversation history in a scrollable pane with
// color-coded messages, tool call blocks, and thinking chains.

use crate::tui::cell::{Cell, Color, Style};
use crate::tui::event::Event;
use crate::tui::layout::Rect;
use crate::tui::screen::Screen;
use crate::tui::widget::{Action, Widget, styles};

/// A single line in the conversation display.
#[derive(Clone, Debug)]
pub enum ConversationLine {
    /// A user message.
    User { text: String },
    /// An assistant message.
    Assistant { text: String },
    /// A tool call header (e.g., "── Tool: read ──").
    ToolCall { name: String, args_summary: String },
    /// A tool result.
    ToolResult {
        name: String,
        content: String,
        is_error: bool,
    },
    /// A system message.
    System { text: String },
    /// Thinking/reasoning chain content.
    Thinking { text: String },
    /// A horizontal separator line.
    Separator,
    /// A confirmation prompt for a tool call, awaiting user y/n/a response.
    ConfirmPrompt { name: String, args_summary: String },
}

/// Scrollable conversation pane.
pub struct ConversationWidget {
    lines: Vec<ConversationLine>,
    /// Scroll offset in **visual row units** (not conversation line units).
    scroll_offset: usize,
    auto_scroll: bool,
}

impl ConversationWidget {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
        }
    }

    /// Add a line to the conversation.
    pub fn push(&mut self, line: ConversationLine) {
        self.lines.push(line);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Add multiple lines at once.
    pub fn extend(&mut self, lines: Vec<ConversationLine>) {
        self.lines.extend(lines);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Clear all conversation lines.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    /// Get a mutable reference to the last line, if any.
    pub fn last_mut(&mut self) -> Option<&mut ConversationLine> {
        self.lines.last_mut()
    }

    /// Check if the last line is an assistant message.
    pub fn last_is_assistant(&self) -> bool {
        matches!(self.lines.last(), Some(ConversationLine::Assistant { .. }))
    }

    /// Calculate how many visual rows a conversation line occupies given the
    /// area width. Uses character-level wrapping to match actual rendering.
    fn line_height(&self, line: &ConversationLine, area_width: u16) -> usize {
        let (text, prefix_len) = match line {
            ConversationLine::User { text } => (text, 7),
            ConversationLine::Assistant { text } => (text, 7),
            ConversationLine::System { text } => (text, 2),
            ConversationLine::Thinking { text } => (text, 13),
            ConversationLine::ToolResult { content, .. } => {
                // For tool results, trim trailing blank lines and limit to
                // a reasonable number of visible lines
                let trimmed = content.trim_end_matches('\n');
                let lines: Vec<&str> = trimmed.lines().collect();
                // Show at most 20 lines for a tool result
                let visible_lines = lines.iter().copied().take(20).collect::<Vec<&str>>();
                let joined = visible_lines.join("\n");
                return self.line_height_for_text(&joined, 4, area_width).max(1); // At least 1 row even for empty content
            }
            ConversationLine::ToolCall { .. } => return 1,
            ConversationLine::Separator => return 1,
            ConversationLine::ConfirmPrompt { .. } => return 1,
        };

        self.line_height_for_text(text, prefix_len, area_width)
    }

    /// Helper: calculate visual row count for text with a given prefix and area width.
    fn line_height_for_text(&self, text: &str, prefix_len: usize, area_width: u16) -> usize {
        if area_width == 0 || text.is_empty() {
            return 1;
        }

        let wrap_col = area_width as usize;
        let mut rows = 1usize;
        let mut col = prefix_len;

        for ch in text.chars() {
            if ch == '\n' {
                rows += 1;
                col = prefix_len;
            } else if col >= wrap_col {
                rows += 1;
                col = prefix_len + 1;
            } else {
                col += 1;
            }
        }
        rows
    }

    /// Calculate the total visual height of all conversation lines.
    fn total_visual_height(&self, width: u16) -> usize {
        self.lines.iter().map(|l| self.line_height(l, width)).sum()
    }

    /// Scroll to the bottom of the conversation.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
        self.auto_scroll = true;
    }

    /// Scroll up by `n` visual rows.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    /// Scroll down by `n` visual rows.
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
    }

    /// Scroll to the very top.
    pub fn scroll_home(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = false;
    }

    /// Render a single conversation line, with clipping support for scroll.
    fn render_line_clipped(
        &self,
        line: &ConversationLine,
        start_row: u16,
        screen: &mut Screen,
        _width: u16,
        area: Rect,
        skip_top: usize,
        rows_available: usize,
    ) {
        let max_row = area.bottom().saturating_sub(1);
        let effective_max_row = (start_row + rows_available as u16)
            .saturating_sub(1)
            .min(max_row);
        // Reserve 1 column for the scrollbar on the right edge
        let wrap_col = area.x + area.width.saturating_sub(1);

        match line {
            ConversationLine::User { text } => {
                let prefix = "  You: ";
                if skip_top == 0 && start_row <= max_row {
                    screen.write_str(
                        start_row,
                        area.x,
                        prefix,
                        Color::GREEN,
                        Color::Default,
                        Style::bold(),
                    );
                    screen.write_str_wrapped_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        text,
                        Color::GREEN,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                    );
                } else if skip_top > 0 {
                    screen.write_str_wrapped_skip_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        text,
                        Color::GREEN,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                        skip_top,
                    );
                }
            }
            ConversationLine::Assistant { text } => {
                let prefix = "  AI:  ";
                if skip_top == 0 && start_row <= max_row {
                    screen.write_str(
                        start_row,
                        area.x,
                        prefix,
                        Color::CYAN,
                        Color::Default,
                        Style::bold(),
                    );
                    screen.write_str_wrapped_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        text,
                        Color::WHITE,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                    );
                } else if skip_top > 0 {
                    screen.write_str_wrapped_skip_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        text,
                        Color::WHITE,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                        skip_top,
                    );
                }
            }
            ConversationLine::ToolCall { name, args_summary } => {
                if skip_top == 0 && start_row <= max_row {
                    let header = format!("  ── {} ", name);
                    screen.write_str(
                        start_row,
                        area.x,
                        &header,
                        styles::TOOL_MSG_FG,
                        Color::Default,
                        Style::default(),
                    );
                    if !args_summary.is_empty() {
                        let available_width = (area.width as usize).saturating_sub(header.len());
                        let args_display = if args_summary.len() > available_width.saturating_sub(3)
                        {
                            let end = available_width.saturating_sub(3).min(args_summary.len());
                            format!("{}...", &args_summary[..end])
                        } else {
                            args_summary.clone()
                        };
                        screen.write_str(
                            start_row,
                            area.x + header.len() as u16,
                            &args_display,
                            Color::Ansi(96),
                            Color::Default,
                            Style::dim(),
                        );
                    }
                }
            }
            ConversationLine::ToolResult {
                name: _,
                content,
                is_error,
            } => {
                let color = if *is_error {
                    Color::RED
                } else {
                    Color::Ansi(252)
                };
                let bg = Color::Ansi(235);
                // Trim trailing blank lines and limit to 20 visible lines
                let trimmed = content.trim_end_matches('\n');
                let content_lines: Vec<&str> = trimmed.lines().take(20).collect();
                let mut current_row = start_row;
                let max_content_width = (area.width as usize).saturating_sub(5);

                for (i, content_line) in content_lines.iter().enumerate() {
                    if i < skip_top {
                        current_row += 1;
                        continue;
                    }
                    if current_row > max_row {
                        break;
                    }
                    let prefix = "  │ ";
                    let display = if content_line.is_empty() {
                        prefix.to_string()
                    } else if content_line.len() > max_content_width {
                        format!(
                            "{}{}…",
                            prefix,
                            &content_line[..max_content_width.saturating_sub(1)]
                        )
                    } else {
                        format!("{}{}", prefix, content_line)
                    };
                    screen.write_str(current_row, area.x, &display, color, bg, Style::default());
                    // Fill background to content width (leaving scrollbar column)
                    let fill_end = area.x + area.width.saturating_sub(1);
                    let end_col = area.x + display.len().min(area.width as usize - 1) as u16;
                    if end_col < fill_end {
                        for c in end_col..fill_end {
                            if let Some(cell) = screen.get_mut(current_row, c) {
                                cell.bg = bg;
                            }
                        }
                    }
                    current_row += 1;
                }
                // If content was truncated, show a truncation indicator
                let total_lines = trimmed.lines().count();
                if total_lines > 20 && skip_top == 0 && current_row <= max_row {
                    let truncation = "  │ …";
                    screen.write_str(
                        current_row,
                        area.x,
                        truncation,
                        Color::Ansi(244),
                        bg,
                        Style::dim(),
                    );
                    let fill_end = area.x + area.width.saturating_sub(1);
                    for c in (area.x + truncation.len() as u16)..fill_end {
                        if let Some(cell) = screen.get_mut(current_row, c) {
                            cell.bg = bg;
                        }
                    }
                }
            }
            ConversationLine::System { text } => {
                if skip_top == 0 && start_row <= max_row {
                    screen.write_str(
                        start_row,
                        area.x,
                        "  ",
                        Color::Default,
                        Color::Default,
                        Style::default(),
                    );
                    screen.write_str_wrapped_clipped(
                        start_row,
                        area.x + 2,
                        text,
                        Color::YELLOW,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                    );
                } else if skip_top > 0 {
                    screen.write_str_wrapped_skip_clipped(
                        start_row,
                        area.x + 2,
                        text,
                        Color::YELLOW,
                        Color::Default,
                        Style::default(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                        skip_top,
                    );
                }
            }
            ConversationLine::Thinking { text } => {
                let prefix = "  [thinking] ";
                if skip_top == 0 && start_row <= max_row {
                    screen.write_str(
                        start_row,
                        area.x,
                        prefix,
                        styles::THINKING_FG,
                        Color::Default,
                        Style::dim(),
                    );
                    let display_text = if text.is_empty() {
                        "⋯".to_string()
                    } else {
                        text.clone()
                    };
                    screen.write_str_wrapped_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        &display_text,
                        styles::THINKING_FG,
                        Color::Default,
                        Style::dim(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                    );
                } else if skip_top > 0 {
                    let display_text = if text.is_empty() {
                        "⋯".to_string()
                    } else {
                        text.clone()
                    };
                    screen.write_str_wrapped_skip_clipped(
                        start_row,
                        area.x + prefix.len() as u16,
                        &display_text,
                        styles::THINKING_FG,
                        Color::Default,
                        Style::dim(),
                        area.x,
                        effective_max_row,
                        wrap_col,
                        skip_top,
                    );
                }
            }
            ConversationLine::Separator => {
                if skip_top == 0 && start_row <= max_row {
                    screen.hline(
                        start_row,
                        area.x + 2,
                        area.x + area.width.saturating_sub(4),
                        '─',
                        Color::Ansi(240),
                        Color::Default,
                    );
                }
            }
            ConversationLine::ConfirmPrompt { name, args_summary } => {
                if skip_top == 0 && start_row <= max_row {
                    let prompt = format!("  ⚠ Confirm {} {}", name, args_summary);
                    let suffix = " [y/n/a]?";
                    screen.write_str(
                        start_row,
                        area.x,
                        &prompt,
                        Color::YELLOW,
                        Color::Default,
                        Style::bold(),
                    );
                    let prompt_end =
                        area.x + prompt.len().min(area.width as usize - suffix.len()) as u16;
                    screen.write_str(
                        start_row,
                        prompt_end,
                        suffix,
                        Color::YELLOW,
                        Color::Default,
                        Style::default(),
                    );
                }
            }
        }
    }

    /// Render a scrollbar on the right edge of the area.
    fn render_scrollbar(&self, area: Rect, screen: &mut Screen, total_lines: usize) {
        if total_lines <= area.height as usize {
            return;
        }

        let scrollbar_height = area.height;
        let thumb_size =
            ((scrollbar_height as usize * scrollbar_height as usize) / total_lines).max(1) as u16;
        let thumb_position = if total_lines > area.height as usize {
            (self.scroll_offset as u16 * (scrollbar_height - thumb_size))
                / (total_lines as u16 - area.height)
        } else {
            0
        };

        let x = area.x + area.width - 1;
        for row in area.y..area.y + area.height {
            if let Some(cell) = screen.get_mut(row, x) {
                cell.char = '│';
                cell.fg = styles::SCROLLBAR_FG;
                cell.bg = styles::SCROLLBAR_BG;
            }
        }

        for i in 0..thumb_size {
            let row = area.y + thumb_position + i;
            if row < area.y + area.height {
                if let Some(cell) = screen.get_mut(row, x) {
                    cell.char = '█';
                    cell.fg = styles::SCROLLBAR_FG;
                }
            }
        }
    }
}

impl Widget for ConversationWidget {
    fn render(&mut self, area: Rect, screen: &mut Screen) {
        if area.is_empty() {
            return;
        }

        screen.fill_rect(area, Cell::default());

        let visible_rows = area.height as usize;
        // Reserve 1 column for the scrollbar
        let content_width = area.width.saturating_sub(1);
        let total_height = self.total_visual_height(content_width);

        let max_scroll = total_height.saturating_sub(visible_rows);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
        if self.auto_scroll {
            self.scroll_offset = max_scroll;
        }

        let mut visual_row = 0usize;
        let mut screen_row = area.y;
        let skip_rows = self.scroll_offset;

        for line in &self.lines {
            let height = self.line_height(line, content_width);

            if visual_row + height <= skip_rows {
                visual_row += height;
                continue;
            }

            let skip_top = skip_rows.saturating_sub(visual_row);
            let rows_available = area.bottom().saturating_sub(screen_row) as usize;

            if rows_available == 0 {
                break;
            }

            self.render_line_clipped(
                line,
                screen_row,
                screen,
                content_width,
                area,
                skip_top,
                rows_available,
            );

            screen_row += height.saturating_sub(skip_top) as u16;
            screen_row = screen_row.min(area.bottom());
            visual_row += height;

            if screen_row >= area.bottom() {
                break;
            }
        }

        self.render_scrollbar(area, screen, total_height);
    }

    fn handle_event(&mut self, event: &Event) -> Action {
        let _ = event;
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_new() {
        let conv = ConversationWidget::new();
        assert!(conv.lines.is_empty());
        assert!(conv.auto_scroll);
    }

    #[test]
    fn test_conversation_push() {
        let mut conv = ConversationWidget::new();
        conv.push(ConversationLine::User {
            text: "Hello".to_string(),
        });
        conv.push(ConversationLine::Assistant {
            text: "Hi there!".to_string(),
        });
        assert_eq!(conv.lines.len(), 2);
    }

    #[test]
    fn test_conversation_scroll() {
        let mut conv = ConversationWidget::new();
        for i in 0..50 {
            conv.push(ConversationLine::User {
                text: format!("Message {}", i),
            });
        }
        assert!(conv.auto_scroll);

        conv.scroll_up(10);
        assert!(!conv.auto_scroll);

        conv.scroll_down(10);
    }

    #[test]
    fn test_conversation_clear() {
        let mut conv = ConversationWidget::new();
        conv.push(ConversationLine::User {
            text: "Hello".to_string(),
        });
        conv.clear();
        assert!(conv.lines.is_empty());
    }

    #[test]
    fn test_conversation_render() {
        let mut screen = Screen::new(80, 24);
        let mut conv = ConversationWidget::new();
        conv.push(ConversationLine::User {
            text: "Hello".to_string(),
        });
        conv.push(ConversationLine::Separator);
        conv.push(ConversationLine::Assistant {
            text: "Hi there!".to_string(),
        });

        let area = Rect::new(0, 0, 80, 20);
        conv.render(area, &mut screen);

        assert!(screen.get(0, 0).unwrap().char != '\0');
    }

    #[test]
    fn test_tool_result_line_height() {
        let conv = ConversationWidget::new();
        let line = ConversationLine::ToolResult {
            name: "read".to_string(),
            content: "line1\nline2\nline3".to_string(),
            is_error: false,
        };
        assert_eq!(conv.line_height(&line, 80), 3);
    }

    #[test]
    fn test_tool_result_trailing_newlines() {
        let conv = ConversationWidget::new();
        // Trailing newlines should be trimmed
        let line = ConversationLine::ToolResult {
            name: "read".to_string(),
            content: "line1\nline2\n\n\n".to_string(),
            is_error: false,
        };
        // Should count 2 visible lines, not 4
        assert_eq!(conv.line_height(&line, 80), 2);
    }

    #[test]
    fn test_tool_result_truncated() {
        let conv = ConversationWidget::new();
        // Content with more than 20 lines should be capped
        let long_content: String = (0..30)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let line = ConversationLine::ToolResult {
            name: "run".to_string(),
            content: long_content,
            is_error: false,
        };
        // Should be capped at 20 lines + truncation indicator
        let height = conv.line_height(&line, 80);
        assert!(
            height <= 21,
            "Tool result should be capped at ~20 lines, got {}",
            height
        );
    }

    #[test]
    fn test_tool_result_single_line() {
        let conv = ConversationWidget::new();
        let line = ConversationLine::ToolResult {
            name: "read".to_string(),
            content: "single line".to_string(),
            is_error: false,
        };
        assert_eq!(conv.line_height(&line, 80), 1);
    }

    #[test]
    fn test_assistant_wrapping_line_height() {
        let conv = ConversationWidget::new();
        // With width=20, prefix "  AI:  " (7 chars), text "12345678901234567890" (20 chars)
        // First row: prefix + 13 chars, second row: 7 more chars
        let line = ConversationLine::Assistant {
            text: "12345678901234567890".to_string(),
        };
        let height = conv.line_height(&line, 20);
        assert!(
            height >= 2,
            "Long text should wrap to multiple rows, got {}",
            height
        );
    }

    #[test]
    fn test_confirm_prompt_line_height() {
        let conv = ConversationWidget::new();
        let line = ConversationLine::ConfirmPrompt {
            name: "run".to_string(),
            args_summary: "cargo test".to_string(),
        };
        // ConfirmPrompt always takes 1 row
        assert_eq!(conv.line_height(&line, 80), 1);
    }

    #[test]
    fn test_confirm_prompt_push() {
        let mut conv = ConversationWidget::new();
        conv.push(ConversationLine::ConfirmPrompt {
            name: "run".to_string(),
            args_summary: "cargo build".to_string(),
        });
        assert_eq!(conv.lines.len(), 1);
    }
}
