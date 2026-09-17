use crate::config::Config;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const UNIT_DIR: &str = "/etc/systemd/system";

/// Finds every unit that's plausibly "your own": a `.service` whose
/// `ExecStart=` binary path lives under `$HOME` (catches every tool this
/// ecosystem has, excludes distro/system services like NetworkManager or
/// sshd), plus any `.timer` that triggers one of those services. Verified
/// live against this box's real unit files before writing this — the
/// `ExecStart=` line in a unit *file* is a plain path
/// (`ExecStart=/home/you/bin/thing --arg`), not the structured
/// `{ path=... ; argv[]=... }` form `systemctl show` prints at runtime.
pub fn discover_units(cfg: &Config) -> Vec<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let home_str = home.to_string_lossy().to_string();

    let mut services = BTreeSet::new();
    let mut timers: Vec<(String, String)> = Vec::new(); // (timer unit, triggered service basename)

    if let Ok(entries) = fs::read_dir(UNIT_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if let Some(base) = name.strip_suffix(".service") {
                if exec_start_under_home(&path, &home_str) {
                    services.insert(base.to_string());
                }
            } else if name.ends_with(".timer") {
                let triggered = timer_target_service(&path)
                    .unwrap_or_else(|| name.trim_end_matches(".timer").to_string());
                timers.push((name.to_string(), triggered));
            }
        }
    }

    let mut units: BTreeSet<String> = services.iter().map(|b| format!("{b}.service")).collect();
    for (timer_unit, triggered_base) in &timers {
        if services.contains(triggered_base) {
            units.insert(timer_unit.clone());
        }
    }

    for extra in &cfg.extra_units {
        units.insert(extra.clone());
    }
    for ignored in &cfg.ignore {
        units.remove(ignored);
    }

    units.into_iter().collect()
}

/// Reads a `.service` file's `ExecStart=` line and checks whether the
/// binary path starts under `$HOME`. A leading `-`/`@`/`+`/`!`/`:` modifier
/// (systemd's own unit-file syntax for "ignore exit code", "argv0
/// override", etc.) is stripped first since it isn't part of the path.
fn exec_start_under_home(unit_file: &Path, home: &str) -> bool {
    let Ok(content) = fs::read_to_string(unit_file) else {
        return false;
    };
    for line in content.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("ExecStart=") else {
            continue;
        };
        let rest = rest.trim_start_matches(['-', '@', '+', '!', ':']);
        let bin_path = rest.split_whitespace().next().unwrap_or("");
        return bin_path.starts_with(home);
    }
    false
}

/// A `.timer`'s `[Timer]` section can name the service it triggers via
/// `Unit=`; if absent, systemd defaults to the same-basename `.service`
/// (handled by the caller's fallback, not here).
fn timer_target_service(timer_file: &Path) -> Option<String> {
    let content = fs::read_to_string(timer_file).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Unit=") {
            return Some(rest.trim_end_matches(".service").to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_file(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cyberwatch_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn exec_start_under_home_matches_real_home_path() {
        let path = scratch_file(
            "own.service",
            "[Service]\nExecStart=/home/raven/.cargo-target/release/thing --flag\n",
        );
        assert!(exec_start_under_home(&path, "/home/raven"));
    }

    #[test]
    fn exec_start_under_home_rejects_system_path() {
        let path = scratch_file(
            "system.service",
            "[Service]\nExecStart=/usr/lib/NetworkManager/nm-iface-helper\n",
        );
        assert!(!exec_start_under_home(&path, "/home/raven"));
    }

    #[test]
    fn exec_start_strips_leading_modifier_char() {
        // systemd unit-file syntax: a leading '-' means "ignore exit code" --
        // not part of the path.
        let path = scratch_file(
            "modifier.service",
            "[Service]\nExecStart=-/home/raven/.cargo-target/release/thing\n",
        );
        assert!(exec_start_under_home(&path, "/home/raven"));
    }

    #[test]
    fn timer_target_service_reads_explicit_unit() {
        let path = scratch_file(
            "custom-name.timer",
            "[Timer]\nOnCalendar=daily\nUnit=sigilward-check.service\n",
        );
        assert_eq!(
            timer_target_service(&path),
            Some("sigilward-check".to_string())
        );
    }

    #[test]
    fn timer_target_service_none_when_unspecified() {
        let path = scratch_file("plain.timer", "[Timer]\nOnCalendar=daily\n");
        assert_eq!(timer_target_service(&path), None);
    }
}
