//! cyberwatch's discovery and status logic as a library, so the TUI, the
//! bar widget's `--summary` and DaemonHall (the web dashboard) all agree on
//! which units are "yours" and which ones need attention.
//!
//! Unit names are plain for system units (`wraithflow.service`) and carry a
//! `user:` prefix for units in the user manager (`user:cyberdesk.service`);
//! see [`unit::Unit`].

pub mod actions;
pub mod config;
pub mod discover;
pub mod status;
pub mod unit;
