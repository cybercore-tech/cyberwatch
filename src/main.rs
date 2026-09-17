mod actions;
mod app;
mod config;
mod discover;
mod status;
mod theme;
mod ui;

use actions::Verb;
use anyhow::Result;
use app::{ActionJob, App, Mode, Pending};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use theme::Theme;

enum ArgMode {
    Tui,
    Summary,
    List,
}

fn parse_args() -> std::result::Result<ArgMode, String> {
    let mut mode = ArgMode::Tui;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--summary" => mode = ArgMode::Summary,
            "--list" => mode = ArgMode::List,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "cyberwatch: unrecognized argument '{other}'\nRun `cyberwatch --help` for usage."
                ));
            }
        }
    }
    Ok(mode)
}

fn print_usage() {
    println!(
        "cyberwatch — a live systemd dashboard for your own services\n\
         \n\
         USAGE:\n\
         \x20   cyberwatch [OPTIONS]\n\
         \n\
         OPTIONS:\n\
         \x20   --summary   Print a one-line JSON status (for the bar widget) and exit\n\
         \x20   --list      Print a full JSON array of every watched unit's status and exit\n\
         \x20   -h, --help  Print this help and exit\n\
         \n\
         Watches units whose ExecStart binary lives under $HOME, plus any\n\
         .timer that triggers one of those services. Config (extra units,\n\
         ignore list): ~/.config/cyberwatch/config.json"
    );
}

/// Fast pass for the bar widget: discover + status every watched unit,
/// print one JSON line, exit.
fn print_summary() -> Result<()> {
    let cfg = config::load_or_init()?;
    let names = discover::discover_units(&cfg);
    let timer_targets: std::collections::BTreeSet<String> = names
        .iter()
        .filter(|n| n.ends_with(".timer"))
        .map(|n| n.trim_end_matches(".timer").to_string())
        .collect();

    let total = names.len();
    let attention = names
        .iter()
        .map(|name| {
            let kind = if name.ends_with(".timer") {
                status::Kind::Timer
            } else {
                status::Kind::Service
            };
            let base = name.trim_end_matches(".service").trim_end_matches(".timer");
            status::unit_status(name, kind, timer_targets.contains(base))
        })
        .filter(|u| u.needs_attention())
        .count();

    println!("{{\"attention\":{attention},\"total\":{total}}}");
    Ok(())
}

fn print_list() -> Result<()> {
    let cfg = config::load_or_init()?;
    let names = discover::discover_units(&cfg);
    let timer_targets: std::collections::BTreeSet<String> = names
        .iter()
        .filter(|n| n.ends_with(".timer"))
        .map(|n| n.trim_end_matches(".timer").to_string())
        .collect();

    let units: Vec<serde_json::Value> = names
        .iter()
        .map(|name| {
            let kind = if name.ends_with(".timer") {
                status::Kind::Timer
            } else {
                status::Kind::Service
            };
            let base = name.trim_end_matches(".service").trim_end_matches(".timer");
            let u = status::unit_status(name, kind, timer_targets.contains(base));
            serde_json::json!({
                "name": u.name,
                "kind": match u.kind { status::Kind::Service => "service", status::Kind::Timer => "timer" },
                "active_state": u.active_state,
                "sub_state": u.sub_state,
                "unit_file_state": u.unit_file_state,
                "result": u.result,
                "needs_attention": u.needs_attention(),
                "status_label": u.status_label(),
            })
        })
        .collect();

    println!("{}", serde_json::to_string(&units)?);
    Ok(())
}

fn main() -> Result<()> {
    let mode = match parse_args() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    match mode {
        ArgMode::Summary => return print_summary(),
        ArgMode::List => return print_list(),
        ArgMode::Tui => {}
    }

    let theme = Theme::from_cybercore();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    let result = run(&mut terminal, &mut app, &theme);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    theme: &Theme,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app, theme))?;

        if let Some(work) = app.pending.take() {
            app.perform(work)?;
            continue;
        }

        if let Some(job) = app.action_job.take() {
            suspend_and_act(terminal, app, job)?;
            continue;
        }

        if app.should_quit {
            return Ok(());
        }

        if let Event::Key(key) = event::read()? {
            match app.mode {
                Mode::Normal => handle_normal(app, key.code, key.modifiers),
                Mode::Filter => handle_filter(app, key.code),
                Mode::Detail => handle_detail(app, key.code),
                Mode::Help => handle_help(app, key.code),
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Leaves the alternate screen and raw mode entirely, runs `sudo systemctl
/// <verb> <unit>` with a real inherited terminal (see `actions::
/// run_inherited` for why), then resumes the TUI. Same shape as
/// cyberfleet's `suspend_and_fetch`.
fn suspend_and_act(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    job: ActionJob,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    let action_label = match job.action {
        app::Action::Systemctl(verb) => {
            println!("\n=== systemctl {} {} ===", verb_str(verb), job.unit);
            match actions::run_inherited(verb, &job.unit) {
                Ok(status) if status.success() => println!("done."),
                Ok(status) => println!("systemctl exited with {status}"),
                Err(e) => println!("failed to run systemctl: {e}"),
            }
            verb_str(verb).to_string()
        }
        app::Action::Edit => {
            println!("\n=== sudoedit {} ===", actions::unit_file_path(&job.unit));
            match actions::edit_inherited(&job.unit) {
                Ok(status) if status.success() => println!("saved (daemon-reload done)."),
                Ok(status) => println!("sudoedit/daemon-reload exited with {status}"),
                Err(e) => println!("failed to edit: {e}"),
            }
            "edit".to_string()
        }
    };
    println!("\nPress Enter to return to cyberwatch.");
    let mut discard = String::new();
    let _ = io::stdin().read_line(&mut discard);

    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    terminal.clear()?;

    app.status = Some(format!("{action_label} {}: refreshing...", job.unit));
    app.pending = Some(Pending::Rescan);
    Ok(())
}

fn verb_str(verb: Verb) -> &'static str {
    match verb {
        Verb::Start => "start",
        Verb::Stop => "stop",
        Verb::Restart => "restart",
    }
}

fn handle_normal(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.previous(),
        KeyCode::Char('/') => app.mode = Mode::Filter,
        KeyCode::Char('R') => {
            app.status = Some("rescanning...".to_string());
            app.pending = Some(Pending::Rescan);
        }
        KeyCode::Char('s') => {
            if let Some(u) = app.selected_unit() {
                app.action_job = Some(ActionJob {
                    action: app::Action::Systemctl(Verb::Start),
                    unit: u.name.clone(),
                });
            }
        }
        KeyCode::Char('x') => {
            if let Some(u) = app.selected_unit() {
                app.action_job = Some(ActionJob {
                    action: app::Action::Systemctl(Verb::Stop),
                    unit: u.name.clone(),
                });
            }
        }
        KeyCode::Char('r') => {
            if let Some(u) = app.selected_unit() {
                app.action_job = Some(ActionJob {
                    action: app::Action::Systemctl(Verb::Restart),
                    unit: u.name.clone(),
                });
            }
        }
        KeyCode::Char('e') => {
            if let Some(u) = app.selected_unit() {
                app.action_job = Some(ActionJob {
                    action: app::Action::Edit,
                    unit: u.name.clone(),
                });
            }
        }
        KeyCode::Char('l') => {
            if let Some(u) = app.selected_unit() {
                app.detail_text = status::journal_tail(&u.name, 200);
                app.mode = Mode::Detail;
            }
        }
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.should_quit = true,
        _ => {}
    }
}

fn handle_filter(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.filter_text.clear();
            app.apply_sort_and_filter();
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => app.mode = Mode::Normal,
        KeyCode::Backspace => {
            app.filter_text.pop();
            app.apply_sort_and_filter();
        }
        KeyCode::Char(c) => {
            app.filter_text.push(c);
            app.apply_sort_and_filter();
        }
        _ => {}
    }
}

fn handle_detail(app: &mut App, code: KeyCode) {
    if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l')) {
        app.mode = Mode::Normal;
    }
}

fn handle_help(app: &mut App, code: KeyCode) {
    if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')) {
        app.mode = Mode::Normal;
    }
}
