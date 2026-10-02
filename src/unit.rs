/// Which systemd manager a unit belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    System,
    User,
}

/// A watched unit as cyberwatch names it: `wraithflow.service` for the
/// system manager, `user:cyberdesk.service` for the user manager. The
/// prefix keeps one flat list (and one `config.json`) for both scopes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Unit {
    pub scope: Scope,
    pub name: String,
}

pub const USER_PREFIX: &str = "user:";

impl Unit {
    pub fn parse(id: &str) -> Unit {
        match id.strip_prefix(USER_PREFIX) {
            Some(name) => Unit {
                scope: Scope::User,
                name: name.to_string(),
            },
            None => Unit {
                scope: Scope::System,
                name: id.to_string(),
            },
        }
    }

    pub fn system(name: &str) -> Unit {
        Unit {
            scope: Scope::System,
            name: name.to_string(),
        }
    }

    pub fn user(name: &str) -> Unit {
        Unit {
            scope: Scope::User,
            name: name.to_string(),
        }
    }

    /// The prefixed form stored in config and shown in lists.
    pub fn id(&self) -> String {
        match self.scope {
            Scope::System => self.name.clone(),
            Scope::User => format!("{USER_PREFIX}{}", self.name),
        }
    }

    pub fn is_user(&self) -> bool {
        self.scope == Scope::User
    }

    /// `--user` for user units, nothing for system ones — prepend to any
    /// `systemctl` argument list.
    pub fn systemctl_scope_args(&self) -> &'static [&'static str] {
        match self.scope {
            Scope::System => &[],
            Scope::User => &["--user"],
        }
    }

    /// `journalctl` selector for this unit's own log lines.
    pub fn journal_args(&self) -> [String; 2] {
        match self.scope {
            Scope::System => ["-u".to_string(), self.name.clone()],
            Scope::User => ["--user-unit".to_string(), self.name.clone()],
        }
    }

    pub fn is_timer(&self) -> bool {
        self.name.ends_with(".timer")
    }

    /// Unit name without its `.service` / `.timer` suffix.
    pub fn base(&self) -> &str {
        self.name
            .trim_end_matches(".service")
            .trim_end_matches(".timer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trips_both_scopes() {
        for id in [
            "wraithflow.service",
            "user:cyberdesk.service",
            "user:pkg-snapshot.timer",
        ] {
            assert_eq!(Unit::parse(id).id(), id);
        }
        assert!(Unit::parse("user:cyberdesk.service").is_user());
        assert!(!Unit::parse("argus.service").is_user());
    }

    #[test]
    fn scope_args_and_journal_selector() {
        let u = Unit::parse("user:cyberdesk.service");
        assert_eq!(u.systemctl_scope_args(), &["--user"]);
        assert_eq!(
            u.journal_args(),
            ["--user-unit".to_string(), "cyberdesk.service".to_string()]
        );
        let s = Unit::parse("argus.service");
        assert!(s.systemctl_scope_args().is_empty());
        assert_eq!(s.journal_args()[0], "-u");
    }

    #[test]
    fn base_strips_suffix() {
        assert_eq!(
            Unit::parse("user:pkg-snapshot.timer").base(),
            "pkg-snapshot"
        );
        assert!(Unit::parse("sigilward-check.timer").is_timer());
    }
}
