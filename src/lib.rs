//! `better-panic` gives you pretty backtraces for panics.
//!
//! It is inspired by Python tracebacks and tries to replicate them as well
//! as possible.  This is what it looks like:
//!
//! <img src="https://github.com/mitsuhiko/better-panic/raw/master/screenshot.png">
//!
//! Some of the code is based on the
//! [color-backtrace](https://crates.io/crates/color-backtrace) library.
//!
//! ## Usage
//!
//! The most common way to use it is to invoke the `install` function
//! which installs a panic handler.  In debug builds the backtrace is shown
//! automatically, in release builds it's hidden by default.
//!
//! ```
//! better_panic::install();
//! ```
//!
//! For more configuration see the `Settings` object.
//!
//! ## Features
//!
//! - Colorize backtraces to be easier on the eyes
//! - Show source snippets if source files are found on disk
//! - Hide all the frames after the panic was already initiated
use console::style;
use std::borrow::Cow;
use std::fs::File;
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::panic::PanicInfo;
use std::path::{Path, PathBuf};

/// Defines how verbose the backtrace is supposed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    /// Print a small message including the panic payload and the panic location.
    Minimal,
    /// Everything in `Minimal` and additionally print a backtrace.
    Medium,
    /// Everything in `Medium` plus source snippets for all backtrace locations.
    Full,
}

impl Verbosity {
    /// Get the verbosity level from the `RUST_BACKTRACE` env variable.
    pub fn from_env() -> Self {
        match std::env::var("RUST_BACKTRACE") {
            Ok(ref x) if x == "full" => Verbosity::Full,
            Ok(_) => Verbosity::Medium,
            Err(_) => Verbosity::Minimal,
        }
    }

    fn apply_to_process(self) {
        let val = match self {
            Verbosity::Full => "full",
            Verbosity::Medium => "1",
            Verbosity::Minimal => "",
        };
        if val.is_empty() {
            std::env::remove_var("RUST_BACKTRACE");
        } else {
            std::env::set_var("RUST_BACKTRACE", val);
        }
    }
}

/// Installs the panic handler with `Settings::auto`.
pub fn install() {
    Settings::auto().install()
}

/// Installs the panic handler with debug settings.
pub fn debug_install() {
    Settings::debug().install()
}

struct Frame {
    name: Option<String>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

impl Frame {
    fn name_without_hash(&self) -> Option<&str> {
        let name = self.name.as_ref()?;
        let has_hash_suffix = name.len() > 19
            && &name[name.len() - 19..name.len() - 16] == "::h"
            && name[name.len() - 16..].chars().all(|x| x.is_digit(16));
        if has_hash_suffix {
            Some(&name[..name.len() - 19])
        } else {
            Some(name)
        }
    }

    fn is_dependency_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "std::",
            "core::",
            "backtrace::backtrace::",
            "_rust_begin_unwind",
            "better_panic::",
            "__rust_",
            "___rust_",
            "__pthread",
            "_main",
            "main",
            "__scrt_common_main_seh",
            "BaseThreadInitThunk",
            "_start",
            "__libc_start_main",
            "start_thread",
        ];

        // Inspect name.
        if let Some(ref name) = self.name {
            if SYM_PREFIXES.iter().any(|x| name.starts_with(x)) {
                return true;
            }
        }

        const FILE_PREFIXES: &[&str] = &[
            "rust:",
            "/rustc/",
            "src/libstd/",
            "src/libpanic_unwind/",
            "src/libtest/",
        ];

        // Inspect filename.
        if let Some(filename) = self.filename.as_ref().and_then(|x| x.to_str()) {
            // some filenames are really weird from macro expansion.  consider
            // them not to be part of the user code.
            if filename.contains('<') {
                return true;
            }
            if FILE_PREFIXES.iter().any(|x| filename.starts_with(x))
                || filename.contains("/.cargo/registry/src/")
            {
                return true;
            }
        }

        false
    }

    // Heuristically determine whether a frame is likely to be a post panic
    // frame.
    //
    // Post panic frames are frames of a functions called after the actual panic
    // is already in progress and don't contain any useful information for a
    // reader of the backtrace.
    fn is_post_panic_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] = &[
            "_rust_begin_unwind",
            "panic_bounds_check",
            "core::result::unwrap_failed",
            "core::panicking::panic_fmt",
            "core::panicking::panic_bounds_check",
            "color_backtrace::create_panic_handler",
            "std::panicking::begin_panic",
            "begin_panic_fmt",
            "rust_begin_panic",
            "panic_bounds_check",
            "panic_fmt",
        ];

        if let Some(filename) = self.filename.as_ref().and_then(|x| x.to_str()) {
            if filename.contains("libcore/panicking.rs") {
                return true;
            }
        }

        match self.name_without_hash() {
            Some(name) => SYM_PREFIXES
                .iter()
                .any(|x| name.starts_with(x) || name.ends_with("__rust_end_short_backtrace")),
            None => false,
        }
    }

    // Heuristically determine whether a frame is likely to be part of language
    // runtime.
    fn is_runtime_init_code(&self) -> bool {
        const SYM_PREFIXES: &[&str] =
            &["std::rt::lang_start::", "test::run_test::run_test_inner::"];

        let (name, file) = match (self.name_without_hash(), self.filename.as_ref()) {
            (Some(name), Some(filename)) => (name, filename.to_string_lossy()),
            _ => return false,
        };

        if SYM_PREFIXES
            .iter()
            .any(|x| name.starts_with(x) || name.ends_with("__rust_start_short_backtrace"))
        {
            return true;
        }

        // For Linux, this is the best rule for skipping test init I found.
        if name == "{{closure}}" && file == "src/libtest/lib.rs" {
            return true;
        }

        false
    }

    /// Is this a call once frame?
    fn is_call_once(&self) -> bool {
        if let Some(name) = self.name_without_hash() {
            name.ends_with("FnOnce::call_once")
        } else {
            false
        }
    }

    fn print_source(&self, s: &Settings) -> Result<(), io::Error> {
        let (lineno, filename) = match (self.lineno, self.filename.as_ref()) {
            (Some(a), Some(b)) => (a, b),
            // Without a line number and file name, we can't sensibly proceed.
            _ => return Ok(()),
        };

        print_source(filename, lineno, s)
    }

    fn print(&self, s: &Settings) -> Result<(), io::Error> {
        let is_dependency_code = self.is_dependency_code();

        let name = self.name_without_hash().unwrap_or("<unknown>");

        // Print function name.
        let mut name_style = console::Style::new();
        if is_dependency_code {
            name_style = name_style.cyan();
        } else {
            name_style = name_style.green();
        }

        // Print source location, if known.
        let file = match &self.filename {
            Some(filename) => trim_filename(filename),
            None => Cow::Borrowed("<unknown>"),
        };

        if s.lineno_suffix {
            writeln!(
                &s.out,
                "  File \"{}:{}\", in {}",
                style(file).underlined(),
                style(self.lineno.unwrap_or(0)).yellow(),
                name_style.apply_to(name)
            )?;
        } else {
            writeln!(
                &s.out,
                "  File \"{}\", line {}, in {}",
                style(file).underlined(),
                style(self.lineno.unwrap_or(0)).yellow(),
                name_style.apply_to(name)
            )?;
        }

        // Maybe print source.
        if s.verbosity >= Verbosity::Full {
            self.print_source(s)?;
        }

        Ok(())
    }
}

/// Configuration for panic printing.
#[derive(Debug, Clone)]
pub struct Settings {
    message: String,
    out: console::Term,
    verbosity: Verbosity,
    backtrace_first: bool,
    most_recent_first: bool,
    lineno_suffix: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            verbosity: Verbosity::from_env(),
            message: "The application panicked (crashed).".to_owned(),
            out: console::Term::stderr(),
            backtrace_first: true,
            most_recent_first: true,
            lineno_suffix: false,
        }
    }
}

impl Settings {
    /// Alias for `Settings::default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Common settings for debugging.
    pub fn debug() -> Self {
        Self::new().verbosity(Verbosity::Full)
    }

    /// In release builds this is `new`, in debug builds this is `debug`.
    pub fn auto() -> Self {
        #[cfg(debug_assertions)]
        {
            Self::debug()
        }
        #[cfg(not(debug_assertions))]
        {
            Self::new()
        }
    }

    /// Controls the "greeting" message of the panic.
    ///
    /// Defaults to `"The application panicked (crashed)"`.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Controls the verbosity level.
    ///
    /// Defaults to `Verbosity::get_env()`.
    pub fn verbosity(mut self, v: Verbosity) -> Self {
        self.verbosity = v;
        self
    }

    /// Controls the backtrace position.
    ///
    /// Defaults to `true` which causes the backtrace to be printed above
    /// the panic.
    pub fn backtrace_first(mut self, value: bool) -> Self {
        self.backtrace_first = value;
        self
    }

    /// Controls the most-recent-first behavior.
    ///
    /// Defaults to `true` which causes the backtrace to be printed above
    /// the panic.
    pub fn most_recent_first(mut self, value: bool) -> Self {
        self.most_recent_first = value;
        self
    }

    /// Append the line number as suffix to the filename.
    ///
    /// Defaults to `false` which causes line numbers to be rendered separately.
    /// Specifically this renders `File "foo.rs:42"` instead of
    /// `File "foo.rs", line 42` which lets some terminals open the editor
    /// at the right location on click.
    pub fn lineno_suffix(mut self, value: bool) -> Self {
        self.lineno_suffix = value;
        self
    }

    /// Consumes the settings and creates a panic handler.
    pub fn create_panic_handler(self) -> Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send> {
        Box::new(move |pi| {
            print_panic_and_backtrace(pi, &self).unwrap();
        })
    }

    /// Installs the panic handler.
    pub fn install(self) {
        self.verbosity.apply_to_process();
        std::panic::set_hook(self.create_panic_handler())
    }
}

fn print_source(filename: &Path, lineno: u32, s: &Settings) -> Result<(), io::Error> {
    let file = match File::open(filename) {
        Ok(file) => file,
        Err(ref e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        e @ Err(_) => e?,
    };

    let reader = BufReader::new(file);
    let source_line = reader.lines().nth((lineno - 1) as usize);
    if let Some(Ok(source_line)) = source_line {
        writeln!(&s.out, "    {}", style(source_line.trim()).dim())?;
    }

    Ok(())
}

fn print_backtrace(bt: Option<&backtrace::Backtrace>, s: &Settings) -> Result<(), io::Error> {
    if s.most_recent_first {
        writeln!(
            &s.out,
            "{}",
            style("Backtrace (most recent call first):").bold()
        )?;
    } else {
        writeln!(
            &s.out,
            "{}",
            style("Backtrace (most recent call last):").bold()
        )?;
    }

    // Collect frame info.
    let mut frames = Vec::new();
    if let Some(bt) = bt {
        for frame in bt.frames() {
            for sym in frame.symbols() {
                frames.push(Frame {
                    name: sym.name().map(|x| x.to_string()),
                    lineno: sym.lineno(),
                    filename: sym.filename().map(|x| x.into()),
                });
            }
        }
    } else {
        backtrace::trace(|x| {
            // TODO: Don't just drop unresolvable frames.
            backtrace::resolve(x.ip(), |sym| {
                frames.push(Frame {
                    name: sym.name().map(|x| x.to_string()),
                    lineno: sym.lineno(),
                    filename: sym.filename().map(|x| x.into()),
                });
            });

            true
        });
    }

    // Try to find where the interesting part starts...
    let top_cutoff = frames
        .iter()
        .rposition(Frame::is_post_panic_code)
        .map(|x| x + 1)
        .unwrap_or(0);

    // Try to find where language init frames start ...
    let bottom_cutoff = frames
        .iter()
        .position(Frame::is_runtime_init_code)
        .map(|x| x - 1)
        .unwrap_or_else(|| frames.len());

    // Turn them into `Frame` objects and print them.
    let mut frames = &frames[top_cutoff..bottom_cutoff];

    if !frames.is_empty() && frames[frames.len() - 1].is_call_once() {
        frames = &frames[..frames.len() - 1];
    }

    if s.most_recent_first {
        for frame in frames {
            frame.print(s)?;
        }
    } else {
        for frame in frames.iter().rev() {
            frame.print(s)?;
        }
    }

    Ok(())
}

fn print_panic_and_backtrace(pi: &PanicInfo, s: &Settings) -> Result<(), io::Error> {
    if s.backtrace_first {
        print_backtrace_info(s)?;
        writeln!(&s.out)?;
    }
    print_panic_info(pi, s)?;
    if !s.backtrace_first {
        writeln!(&s.out)?;
        print_backtrace_info(s)?;
    }
    Ok(())
}

fn trim_filename(file: &Path) -> Cow<'_, str> {
    let filename = file.to_str().unwrap_or("<bad utf8>");
    if filename.starts_with("/rustc/") {
        if let Some(filename) = filename.get(48..) {
            Cow::Owned(format!("rust:{}", filename))
        } else {
            Cow::Borrowed(filename)
        }
    } else if let Some(basename) = file.file_name().and_then(|x| x.to_str()) {
        if basename.starts_with('<') && basename.ends_with('>') {
            Cow::Borrowed(basename)
        } else {
            Cow::Borrowed(filename)
        }
    } else {
        Cow::Borrowed(filename)
    }
}

fn print_panic_info(pi: &PanicInfo, s: &Settings) -> Result<(), io::Error> {
    writeln!(&s.out, "{}", style(&s.message).bold())?;

    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>");

    // Print panic message.
    let payload = pi
        .payload()
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
        .unwrap_or("Box<Any>");

    for line in payload.lines() {
        writeln!(&s.out, "  {}", style(line).yellow())?;
    }

    // If known, print panic location.
    write!(&s.out, "in ")?;
    if let Some(loc) = pi.location() {
        if s.lineno_suffix {
            writeln!(
                &s.out,
                "{}:{}",
                style(trim_filename(Path::new(loc.file()))).underlined(),
                style(loc.line()).yellow()
            )?;
        } else {
            writeln!(
                &s.out,
                "{}, line {}",
                style(trim_filename(Path::new(loc.file()))).underlined(),
                style(loc.line()).yellow()
            )?;
        }
    } else {
        writeln!(&s.out, "<unknown>")?;
    }
    writeln!(&s.out, "thread: {}", style(thread_name).yellow())?;
    Ok(())
}

fn print_backtrace_info(s: &Settings) -> Result<(), io::Error> {
    // Print some info on how to increase verbosity.
    if s.verbosity == Verbosity::Minimal {
        writeln!(
            &s.out,
            "\nBacktrace omitted. Run with RUST_BACKTRACE=1 to display it."
        )?;
    }
    if s.verbosity <= Verbosity::Medium {
        if s.verbosity == Verbosity::Medium {
            // If exactly medium, no newline was printed before.
            writeln!(&s.out)?;
        }

        writeln!(
            &s.out,
            "Run with RUST_BACKTRACE=full to include source snippets."
        )?;
    }

    if s.verbosity >= Verbosity::Medium {
        print_backtrace(None, s)?;
    }

    Ok(())
}
