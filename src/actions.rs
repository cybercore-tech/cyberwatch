use anyhow::Result;
use std::process::{Command, ExitStatus};

#[derive(Clone, Copy)]
pub enum Verb {
    Start,
    Stop,
    Restart,
}

impl Verb {
    fn as_str(self) -> &'static str {
        match self {
            Verb::Start => "start",
            Verb::Stop => "stop",
            Verb::Restart => "restart",
        }
    }
}

/// Runs `sudo systemctl <verb> <unit>` with the parent's real stdio
/// inherited, not captured — `sudo` asks for a password on the controlling
/// terminal, the same reason cyberfleet suspends the TUI for `git fetch`'s
/// SSH prompt (see its `git_ops::fetch_inherited`). Only call this while
/// the TUI has actually left the alternate screen/raw mode — called while
/// it still owns the terminal, the prompt has nowhere sane to go and the
/// app hangs with its own keystrokes swallowed by `sudo`.
pub fn run_inherited(verb: Verb, unit: &str) -> Result<ExitStatus> {
    Ok(Command::new("sudo")
        .arg("systemctl")
        .arg(verb.as_str())
        .arg(unit)
        .status()?)
}
