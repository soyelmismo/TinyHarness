use std::io::{self, Write};

use crate::style::*;

/// Structured output writer — all terminal I/O goes through this.
///
/// Provides plain output methods (`line`, `blank`, `raw`), styled convenience
/// methods (`success`, `error`, `warning`, `info`, `dim`, `bold`), and
/// implements [`Write`] so the standard `write!`/`writeln!` macros work
/// directly with ANSI style constants.
///
/// # Examples
///
/// ```rust
/// use tinyharness_ui::output::Output;
/// use std::io::Write;
///
/// let mut out = Output::stdout();
/// let _ = writeln!(out, "Hello, world!");
/// out.success("Done.").unwrap();
/// ```
pub struct Output {
    writer: Box<dyn Write + Send>,
}

impl Output {
    /// Create an `Output` that writes to stdout.
    pub fn stdout() -> Self {
        Self {
            writer: Box::new(io::stdout()),
        }
    }

    /// Create an `Output` that writes to stderr.
    pub fn stderr() -> Self {
        Self {
            writer: Box::new(io::stderr()),
        }
    }

    /// Create an `Output` with a custom writer (e.g. a `Vec<u8>` for testing).
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self { writer }
    }

    // ── Plain output ──

    /// Write text followed by a newline.
    ///
    /// Returns `io::Result<()>`; callers may use `let _ = out.line("...");`
    /// to ignore errors when writing to stdout/stderr (where failures are rare
    /// and typically fatal anyway).
    pub fn line(&mut self, text: &str) -> io::Result<()> {
        writeln!(self, "{text}")
    }

    /// Write a blank line (just a newline).
    pub fn blank(&mut self) -> io::Result<()> {
        writeln!(self)
    }

    /// Write raw bytes as-is (no newline appended, no styling applied).
    pub fn raw(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)
    }

    // ── Styled single-line messages ──

    /// Write a success message (green) followed by a newline.
    pub fn success(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{GREEN}{msg}{RESET}")
    }

    /// Write an error message (red) followed by a newline.
    pub fn error(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{RED}{msg}{RESET}")
    }

    /// Write a warning message (orange) followed by a newline.
    pub fn warning(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{ORANGE}{msg}{RESET}")
    }

    /// Write an info message (blue) followed by a newline.
    pub fn info(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{BLUE}{msg}{RESET}")
    }

    /// Write a dim/gray message followed by a newline.
    pub fn dim(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{GRAY}{msg}{RESET}")
    }

    /// Write a bold message followed by a newline.
    pub fn bold(&mut self, msg: &str) -> io::Result<()> {
        writeln!(self, "{BOLD}{msg}{RESET}")
    }

    /// Write a line with custom prefix, text, and suffix, followed by a newline.
    ///
    /// Typically used with style constants:
    /// ```ignore
    /// out.styled_line(BOLD, "Header:", RESET);
    /// ```
    pub fn styled_line(&mut self, prefix: &str, text: &str, suffix: &str) -> io::Result<()> {
        writeln!(self, "{prefix}{text}{suffix}")
    }
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    /// A writer that captures output into a shared `Vec<u8>` so tests can
    /// inspect what was written.
    #[derive(Clone)]
    struct TestWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl TestWriter {
        fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
            let buf = Arc::new(Mutex::new(Vec::new()));
            let writer = TestWriter { buf: buf.clone() };
            (writer, buf)
        }
    }

    impl Write for TestWriter {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Helper: create an `Output` and the shared buffer it writes into.
    fn captured_output() -> (Output, Arc<Mutex<Vec<u8>>>) {
        let (writer, buf) = TestWriter::new();
        let output = Output::new(Box::new(writer));
        (output, buf)
    }

    /// Get captured output as a String (both raw bytes and ANSI-stripped).
    fn captured_string(buf: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(buf.lock().unwrap().clone()).unwrap()
    }

    /// Strip ANSI SGR sequences from a string for content assertions.
    fn strip_ansi(s: &str) -> String {
        let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        re.replace_all(s, "").to_string()
    }

    // ── Plain output tests ──

    #[test]
    fn line_writes_text_with_newline() {
        let (mut out, buf) = captured_output();
        out.line("hello").unwrap();
        let raw = captured_string(&buf);
        assert_eq!(raw, "hello\n");
    }

    #[test]
    fn line_empty_string() {
        let (mut out, buf) = captured_output();
        out.line("").unwrap();
        assert_eq!(captured_string(&buf), "\n");
    }

    #[test]
    fn blank_writes_newline() {
        let (mut out, buf) = captured_output();
        out.blank().unwrap();
        assert_eq!(captured_string(&buf), "\n");
    }

    #[test]
    fn raw_writes_bytes_exactly() {
        let (mut out, buf) = captured_output();
        out.raw(b"no newline").unwrap();
        assert_eq!(captured_string(&buf), "no newline");
    }

    #[test]
    fn raw_empty_bytes() {
        let (mut out, buf) = captured_output();
        out.raw(b"").unwrap();
        assert!(captured_string(&buf).is_empty());
    }

    #[test]
    fn raw_no_newline_appended() {
        let (mut out, buf) = captured_output();
        out.raw(b"abc").unwrap();
        // Should end exactly with 'c', no trailing newline
        assert!(!captured_string(&buf).ends_with('\n'));
    }

    // ── Styled method tests ──

    #[test]
    fn success_wraps_in_green() {
        let (mut out, buf) = captured_output();
        out.success("OK").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(GREEN), "should contain GREEN ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "OK\n");
    }

    #[test]
    fn error_wraps_in_red() {
        let (mut out, buf) = captured_output();
        out.error("FAIL").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(RED), "should contain RED ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "FAIL\n");
    }

    #[test]
    fn warning_wraps_in_orange() {
        let (mut out, buf) = captured_output();
        out.warning("careful").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(ORANGE), "should contain ORANGE ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "careful\n");
    }

    #[test]
    fn info_wraps_in_blue() {
        let (mut out, buf) = captured_output();
        out.info("note").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(BLUE), "should contain BLUE ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "note\n");
    }

    #[test]
    fn dim_wraps_in_gray() {
        let (mut out, buf) = captured_output();
        out.dim("subtle").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(GRAY), "should contain GRAY ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "subtle\n");
    }

    #[test]
    fn bold_wraps_in_bold() {
        let (mut out, buf) = captured_output();
        out.bold("loud").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(BOLD), "should contain BOLD ANSI code");
        assert!(raw.contains(RESET), "should contain RESET");
        assert_eq!(strip_ansi(&raw), "loud\n");
    }

    #[test]
    fn styled_line_custom_prefix_and_suffix() {
        let (mut out, buf) = captured_output();
        out.styled_line("[", "body", "]").unwrap();
        assert_eq!(captured_string(&buf), "[body]\n");
    }

    #[test]
    fn styled_line_with_ansi_codes() {
        let (mut out, buf) = captured_output();
        out.styled_line(BOLD, "Header", RESET).unwrap();
        let raw = captured_string(&buf);
        assert!(raw.starts_with(BOLD));
        assert!(raw.trim_end().ends_with(RESET));
        assert_eq!(strip_ansi(&raw), "Header\n");
    }

    // ── Write trait tests ──

    #[test]
    fn writeln_macro_works() {
        let (mut out, buf) = captured_output();
        let _ = writeln!(out, "formatted {}", 42);
        assert_eq!(captured_string(&buf), "formatted 42\n");
    }

    #[test]
    fn write_macro_no_newline() {
        let (mut out, buf) = captured_output();
        let _ = write!(out, "no newline");
        assert_eq!(captured_string(&buf), "no newline");
    }

    #[test]
    fn writeln_with_ansi_constants() {
        let (mut out, buf) = captured_output();
        let _ = writeln!(out, "{GREEN}ok{RESET}");
        let raw = captured_string(&buf);
        assert!(raw.contains(GREEN));
        assert!(raw.contains(RESET));
        assert_eq!(strip_ansi(&raw), "ok\n");
    }

    #[test]
    fn write_then_flush() {
        let (mut out, buf) = captured_output();
        let _ = write!(out, "partial");
        // flush should succeed (it's a no-op on our TestWriter)
        assert!(out.flush().is_ok());
        assert_eq!(captured_string(&buf), "partial");
    }

    #[test]
    fn writeln_empty() {
        let (mut out, buf) = captured_output();
        let _ = writeln!(out);
        assert_eq!(captured_string(&buf), "\n");
    }

    // ── Chaining / multiple writes ──

    #[test]
    fn multiple_operations_append() {
        let (mut out, buf) = captured_output();
        out.line("first").unwrap();
        out.success("ok").unwrap();
        let _ = writeln!(out, "{BOLD}bold{RESET}");
        let raw = captured_string(&buf);
        let plain = strip_ansi(&raw);
        assert_eq!(plain, "first\nok\nbold\n");
    }

    #[test]
    fn blank_between_lines() {
        let (mut out, buf) = captured_output();
        out.line("a").unwrap();
        out.blank().unwrap();
        out.line("b").unwrap();
        assert_eq!(captured_string(&buf), "a\n\nb\n");
    }

    // ── Edge cases ──

    #[test]
    fn success_empty_message() {
        let (mut out, buf) = captured_output();
        out.success("").unwrap();
        let raw = captured_string(&buf);
        assert!(raw.contains(GREEN));
        assert!(raw.contains(RESET));
        assert_eq!(strip_ansi(&raw), "\n");
    }

    #[test]
    fn unicode_text() {
        let (mut out, buf) = captured_output();
        out.line("zażółć gęślą jaźń 🦀").unwrap();
        let raw = captured_string(&buf);
        assert_eq!(raw, "zażółć gęślą jaźń 🦀\n");
    }

    #[test]
    fn long_text_no_panic() {
        let (mut out, buf) = captured_output();
        let long = "x".repeat(10_000);
        out.line(&long).unwrap();
        assert_eq!(captured_string(&buf).len(), 10_001); // + newline
    }

    #[test]
    fn stdout_constructs() {
        // Just verify the constructor doesn't panic.
        let _out = Output::stdout();
    }

    #[test]
    fn stderr_constructs() {
        // Just verify the constructor doesn't panic.
        let _out = Output::stderr();
    }

    #[test]
    fn new_with_boxed_writer() {
        // Use new() with a plain Vec<u8> — ensures the constructor accepts any Write + Send.
        let buf = Box::new(Vec::new());
        let mut out = Output::new(buf);
        out.line("test").unwrap();
    }
}
