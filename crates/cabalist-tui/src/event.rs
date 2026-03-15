//! Event handling — terminal input polling and app-level event types.

use crossterm::event::{self, Event as CrosstermEvent};
use std::time::Duration;

/// Events the application loop processes.
#[allow(dead_code)]
pub enum AppEvent {
    /// A terminal event (key press, mouse, resize).
    Terminal(CrosstermEvent),
    /// A periodic tick for housekeeping (status message expiry, etc.).
    Tick,
    /// A line of build output was received.
    BuildOutput(String),
    /// A build process completed. Carries whether it succeeded.
    BuildComplete(bool),
}

/// Poll for the next event. Returns `Tick` if no terminal event arrives within
/// `tick_rate`.
pub fn poll_event(tick_rate: Duration) -> std::io::Result<AppEvent> {
    if event::poll(tick_rate)? {
        let evt = event::read()?;
        Ok(AppEvent::Terminal(evt))
    } else {
        Ok(AppEvent::Tick)
    }
}
