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

/// The real on-disk unit file path this box installs its own units at
/// (see `discover.rs` — the same directory it scans).
pub fn unit_file_path(unit: &str) -> String {
    format!("/etc/systemd/system/{unit}")
}

/// Opens the unit file in `sudoedit` (not `sudo $EDITOR` directly) — it
/// edits a private temp copy as your own user and only copies it back over
/// the root-owned original on save, so a malicious or misbehaving editor
/// plugin never actually runs as root. Same inherited-stdio reasoning as
/// `run_inherited`: the sudo password prompt, and the editor itself, both
/// need a real terminal. Runs `systemctl daemon-reload` afterward
/// regardless of whether the file actually changed — cheap, and skipping
/// it after a real edit is the classic "I changed the unit file and
/// nothing happened" gotcha.
pub fn edit_inherited(unit: &str) -> Result<ExitStatus> {
    let path = unit_file_path(unit);
    let edit_status = Command::new("sudoedit").arg(&path).status()?;
    if !edit_status.success() {
        return Ok(edit_status);
    }
    Command::new("sudo")
        .arg("systemctl")
        .arg("daemon-reload")
        .status()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_file_path_matches_discover_scan_dir() {
        // Must stay in sync with discover.rs's UNIT_DIR — this is exactly
        // the directory the dashboard's own auto-discovery scans.
        assert_eq!(
            unit_file_path("wraithflow.service"),
            "/etc/systemd/system/wraithflow.service"
        );
    }
}
