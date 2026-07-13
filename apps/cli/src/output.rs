use std::{
    env,
    fs::{File, OpenOptions},
    io::{self, IsTerminal, Write},
    path::Path,
    process,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{Context, Result};
use chrono::Utc;
use clap::ValueEnum;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone)]
pub struct Output {
    inner: Arc<OutputInner>,
}

struct OutputInner {
    verbosity: Verbosity,
    color: bool,
    invocation: String,
    run_sequence: AtomicU64,
    human: Mutex<Box<dyn Write + Send>>,
    log: Option<Mutex<File>>,
}

fn color_enabled(choice: ColorChoice, terminal: bool, no_color: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => terminal && !no_color,
    }
}

impl Output {
    pub fn new(verbosity: Verbosity, color: ColorChoice, log_path: Option<&Path>) -> Result<Self> {
        let color = color_enabled(
            color,
            io::stderr().is_terminal(),
            env::var_os("NO_COLOR").is_some(),
        );
        Self::with_writer(verbosity, color, Box::new(io::stderr()), log_path)
    }

    #[cfg(test)]
    fn with_file_writer(
        verbosity: Verbosity,
        color: ColorChoice,
        human_path: &Path,
        log_path: Option<&Path>,
    ) -> Result<Self> {
        let human = Box::new(File::create(human_path)?);
        Self::with_writer(
            verbosity,
            matches!(color, ColorChoice::Always),
            human,
            log_path,
        )
    }

    fn with_writer(
        verbosity: Verbosity,
        color: bool,
        human: Box<dyn Write + Send>,
        log_path: Option<&Path>,
    ) -> Result<Self> {
        let log = log_path
            .map(|path| {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .with_context(|| format!("open diagnostic log {}", path.display()))
                    .map(Mutex::new)
            })
            .transpose()?;
        Ok(Self {
            inner: Arc::new(OutputInner {
                verbosity,
                color,
                invocation: format!("{}-{}", Utc::now().timestamp_millis(), process::id()),
                run_sequence: AtomicU64::new(0),
                human: Mutex::new(human),
                log,
            }),
        })
    }

    pub fn next_run_id(&self) -> String {
        let sequence = self.inner.run_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{}-{sequence}", self.inner.invocation)
    }

    pub fn info(&self, stage: &str, message: &str, metadata: Value) -> Result<()> {
        self.emit("info", stage, message, metadata, false)
    }

    pub fn success(&self, stage: &str, message: &str, metadata: Value) -> Result<()> {
        self.emit("success", stage, message, metadata, false)
    }

    pub fn detail(&self, stage: &str, message: &str, metadata: Value) -> Result<()> {
        self.emit("detail", stage, message, metadata, true)
    }

    pub fn error(&self, stage: &str, message: &str, metadata: Value) -> Result<()> {
        self.emit("error", stage, message, metadata, false)
    }

    fn emit(
        &self,
        level: &str,
        stage: &str,
        message: &str,
        metadata: Value,
        detail: bool,
    ) -> Result<()> {
        let show = level == "error"
            || (self.inner.verbosity != Verbosity::Quiet
                && (!detail || self.inner.verbosity == Verbosity::Verbose));
        if show {
            let code = match level {
                "error" => "31",
                "success" => "32",
                "detail" => "90",
                _ => "36",
            };
            let line = if self.inner.color {
                format!("\x1b[{code}m{message}\x1b[0m\n")
            } else {
                format!("{message}\n")
            };
            self.inner
                .human
                .lock()
                .unwrap()
                .write_all(line.as_bytes())?;
        }
        if let Some(log) = &self.inner.log {
            let mut event = match metadata {
                Value::Object(map) => map,
                _ => serde_json::Map::new(),
            };
            event.insert("ts".into(), json!(Utc::now().to_rfc3339()));
            event.insert("invocation".into(), json!(self.inner.invocation));
            event.insert("level".into(), json!(level));
            event.insert("stage".into(), json!(stage));
            let mut log = log.lock().unwrap();
            serde_json::to_writer(&mut *log, &event)?;
            log.write_all(b"\n")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn colour_choice_honours_no_color_and_explicit_override() {
        assert!(!color_enabled(ColorChoice::Auto, true, true));
        assert!(color_enabled(ColorChoice::Always, true, true));
        assert!(!color_enabled(ColorChoice::Never, true, false));
        assert!(!color_enabled(ColorChoice::Auto, false, false));
    }

    #[test]
    fn reporter_respects_quiet_verbose_and_colour() {
        let dir = tempfile::tempdir().unwrap();
        let human = dir.path().join("human.txt");
        let output =
            Output::with_file_writer(Verbosity::Normal, ColorChoice::Always, &human, None).unwrap();

        output.info("run.start", "starting", json!({})).unwrap();
        output.detail("action.ok", "hidden", json!({})).unwrap();
        output.error("run.failed", "failed", json!({})).unwrap();

        let text = fs::read_to_string(human).unwrap();
        assert!(text.contains("\u{1b}[36mstarting\u{1b}[0m"));
        assert!(!text.contains("hidden"));
        assert!(text.contains("\u{1b}[31mfailed\u{1b}[0m"));
    }

    #[test]
    fn reporter_assigns_distinct_run_ids() {
        let dir = tempfile::tempdir().unwrap();
        let human = dir.path().join("human.txt");
        let output =
            Output::with_file_writer(Verbosity::Quiet, ColorChoice::Never, &human, None).unwrap();

        assert_ne!(output.next_run_id(), output.next_run_id());
    }

    #[test]
    fn reporter_appends_metadata_only_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let human = dir.path().join("human.txt");
        let log = dir.path().join("events.jsonl");

        for message in ["first", "second"] {
            let output =
                Output::with_file_writer(Verbosity::Quiet, ColorChoice::Never, &human, Some(&log))
                    .unwrap();
            output
                .info(
                    "source.complete",
                    message,
                    json!({"workspace":"work","source":"jira","items":2}),
                )
                .unwrap();
        }

        let text = fs::read_to_string(log).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(event["stage"], "source.complete");
        assert_eq!(event["workspace"], "work");
        assert_eq!(event["items"], 2);
        assert!(event.get("message").is_none());
        assert!(event.get("stdout").is_none());
        assert!(event.get("stderr").is_none());
    }
}
