use anyhow::Result;
use cyberwatch::actions::Verb;
use cyberwatch::config::{self, Config};
use cyberwatch::discover;
use cyberwatch::status::{self, Kind, UnitStatus};

pub enum Mode {
    Normal,
    Filter,
    Detail,
    Help,
}

/// Blocking work deferred by one redraw so a key handler never freezes the
/// screen — same pattern as cyberfleet's `Pending`.
pub enum Pending {
    Rescan,
}

/// `sudo systemctl <verb> <unit>` (or `sudoedit` on the unit file) needs a
/// real terminal for its password prompt / editor session — the exact
/// same reason cyberfleet suspends the TUI for `git fetch`'s SSH prompt
/// rather than running it as ordinary `Pending` work.
pub enum Action {
    Systemctl(Verb),
    Edit,
}

pub struct ActionJob {
    pub action: Action,
    pub unit: String,
}

pub struct App {
    pub units: Vec<UnitStatus>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter_text: String,
    pub mode: Mode,
    pub status: Option<String>,
    pub should_quit: bool,
    pub pending: Option<Pending>,
    pub action_job: Option<ActionJob>,
    pub detail_text: String,
    _config: Config,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = config::load_or_init()?;
        Ok(Self {
            units: vec![],
            filtered: vec![],
            selected: 0,
            filter_text: String::new(),
            mode: Mode::Normal,
            status: Some("scanning...".to_string()),
            should_quit: false,
            pending: Some(Pending::Rescan),
            action_job: None,
            detail_text: String::new(),
            _config: config,
        })
    }

    pub fn perform(&mut self, work: Pending) -> Result<()> {
        match work {
            Pending::Rescan => {
                self.rescan();
                let attention = self.units.iter().filter(|u| u.needs_attention()).count();
                self.status = Some(format!(
                    "{} units — {} need attention",
                    self.units.len(),
                    attention
                ));
            }
        }
        Ok(())
    }

    pub fn rescan(&mut self) {
        let names = discover::discover_units(&self._config);
        let timer_targets: std::collections::BTreeSet<String> = names
            .iter()
            .filter(|n| n.ends_with(".timer"))
            .map(|n| n.trim_end_matches(".timer").to_string())
            .collect();

        self.units = names
            .iter()
            .map(|name| {
                let kind = if name.ends_with(".timer") {
                    Kind::Timer
                } else {
                    Kind::Service
                };
                let base = name.trim_end_matches(".service").trim_end_matches(".timer");
                let has_timer = timer_targets.contains(base);
                status::unit_status(name, kind, has_timer)
            })
            .collect();
        self.apply_sort_and_filter();
    }

    /// Needs-attention units first, then alphabetical — same rationale as
    /// cyberfleet: the thing you should look at belongs at the top.
    pub fn apply_sort_and_filter(&mut self) {
        self.units.sort_by(|a, b| {
            b.needs_attention()
                .cmp(&a.needs_attention())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        let needle = self.filter_text.to_lowercase();
        self.filtered = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| needle.is_empty() || u.name.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn selected_unit(&self) -> Option<&UnitStatus> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.units.get(i))
    }

    pub fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }
}
