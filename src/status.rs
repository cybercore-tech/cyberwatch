use crate::unit::Unit;
use std::collections::HashMap;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Service,
    Timer,
}

pub struct UnitStatus {
    pub name: String,
    pub kind: Kind,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    /// `Result=` — services only ("success", "exit-code", "timeout", ...).
    /// Empty for timers, or for a service that's never finished a run.
    pub result: String,
    /// True if a `.timer` triggers this `.service` (see `discover.rs`) —
    /// changes what "needs attention" means, since a timer-triggered
    /// service being `inactive` between runs is completely normal.
    pub has_timer: bool,
}

impl UnitStatus {
    /// The one boolean the bar widget's count and the TUI's sort order
    /// both key off — mirrors cyberfleet's `RepoStatus::needs_attention`.
    pub fn needs_attention(&self) -> bool {
        if self.load_state == "not-found" {
            return true;
        }
        match self.kind {
            // A disabled timer that isn't running is switched off on
            // purpose; only a failed one, or an enabled one that isn't
            // active, is a problem.
            Kind::Timer => {
                self.active_state == "failed"
                    || (self.unit_file_state == "enabled" && self.active_state != "active")
            }
            Kind::Service => {
                if self.active_state == "failed" {
                    return true;
                }
                if self.has_timer {
                    // Timer-triggered: `inactive` between runs is normal —
                    // only the last run's actual result matters.
                    !self.result.is_empty() && self.result != "success"
                } else {
                    self.unit_file_state == "enabled" && self.active_state != "active"
                }
            }
        }
    }

    pub fn status_label(&self) -> &'static str {
        if self.load_state == "not-found" {
            return "MISSING";
        }
        match (self.active_state.as_str(), self.sub_state.as_str()) {
            ("failed", _) => "FAILED",
            ("active", "running") => "running",
            ("active", "waiting") => "waiting",
            ("active", _) => "active",
            ("activating", _) => "starting",
            ("deactivating", _) => "stopping",
            ("inactive", _) => "inactive",
            _ => "unknown",
        }
    }
}

/// One `systemctl show` call per unit (`--user` for `user:` units), parsed as real `KEY=VALUE` lines —
/// deliberately NOT `--value` with multiple `--property=` flags: verified
/// live that systemd prints those in its own fixed property order, not the
/// order they're passed on the command line, so a positional read would
/// silently pair the wrong value with the wrong field.
pub fn unit_status(name: &str, kind: Kind, has_timer: bool) -> UnitStatus {
    let unit = Unit::parse(name);
    let out = Command::new("systemctl")
        .args(unit.systemctl_scope_args())
        .arg("show")
        .arg(&unit.name)
        .arg("--property=LoadState")
        .arg("--property=ActiveState")
        .arg("--property=SubState")
        .arg("--property=UnitFileState")
        .arg("--property=Result")
        .output();

    let mut fields: HashMap<String, String> = HashMap::new();
    if let Ok(out) = out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                fields.insert(k.to_string(), v.to_string());
            }
        }
    }

    let get = |k: &str| fields.get(k).cloned().unwrap_or_default();

    UnitStatus {
        name: name.to_string(),
        kind,
        load_state: get("LoadState"),
        active_state: get("ActiveState"),
        sub_state: get("SubState"),
        unit_file_state: get("UnitFileState"),
        result: get("Result"),
        has_timer,
    }
}

/// Last `n` journal lines for a unit — read-only, no sudo needed (journald
/// grants read access to the `systemd-journal`/`adm` groups, or the unit's
/// own logs if same-user; captured, not inherited, since this is display-
/// only and never prompts for input).
pub fn journal_tail(unit: &str, n: u32) -> String {
    let out = Command::new("journalctl")
        .args(Unit::parse(unit).journal_args())
        .arg("-n")
        .arg(n.to_string())
        .arg("--no-pager")
        .arg("-o")
        .arg("short-iso")
        .output();
    match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        Ok(out) => format!(
            "journalctl exited with {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => format!("failed to run journalctl: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(
        kind: Kind,
        active: &str,
        unit_file: &str,
        result: &str,
        has_timer: bool,
    ) -> UnitStatus {
        UnitStatus {
            name: "test.unit".to_string(),
            kind,
            load_state: "loaded".to_string(),
            active_state: active.to_string(),
            sub_state: String::new(),
            unit_file_state: unit_file.to_string(),
            result: result.to_string(),
            has_timer,
        }
    }

    #[test]
    fn failed_service_always_needs_attention() {
        assert!(unit(Kind::Service, "failed", "enabled", "", false).needs_attention());
        assert!(unit(Kind::Service, "failed", "enabled", "", true).needs_attention());
    }

    #[test]
    fn enabled_inactive_daemon_needs_attention() {
        // A long-running daemon (no paired timer) that's enabled but not
        // active means "should be running, isn't."
        assert!(unit(Kind::Service, "inactive", "enabled", "", false).needs_attention());
    }

    #[test]
    fn disabled_inactive_service_is_fine() {
        assert!(!unit(Kind::Service, "inactive", "disabled", "", false).needs_attention());
    }

    #[test]
    fn timer_triggered_service_inactive_is_normal() {
        // Between runs, a timer-triggered service is *supposed* to be
        // inactive -- only a real bad `Result=` should flag it.
        assert!(!unit(Kind::Service, "inactive", "enabled", "success", true).needs_attention());
        assert!(unit(Kind::Service, "inactive", "enabled", "exit-code", true).needs_attention());
    }

    #[test]
    fn timer_needs_attention_when_not_active() {
        assert!(!unit(Kind::Timer, "active", "enabled", "", false).needs_attention());
        assert!(unit(Kind::Timer, "failed", "enabled", "", false).needs_attention());
        assert!(unit(Kind::Timer, "inactive", "enabled", "", false).needs_attention());
    }

    #[test]
    fn disabled_inactive_timer_is_fine() {
        assert!(!unit(Kind::Timer, "inactive", "disabled", "", false).needs_attention());
        assert!(unit(Kind::Timer, "failed", "disabled", "", false).needs_attention());
    }

    #[test]
    fn missing_unit_always_needs_attention() {
        let mut u = unit(Kind::Service, "inactive", "", "", false);
        u.load_state = "not-found".to_string();
        assert!(u.needs_attention());
    }
}
