//! Top-level render entry point for the terminal frontend.
//!
//! `draw` is the single function called each frame by the main loop.  It
//! paints the background and dispatches to whichever per-screen module owns
//! the current [`Screen`].

use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};
use ratatui::Frame;

use crate::app::{App, Screen};

use super::*;

pub fn draw(f: &mut Frame, app: &App, fx: &Transitions) {
    let area = f.area();
    Block::default()
        .style(Style::default().bg(BG))
        .render(area, f.buffer_mut());

    match app.screen {
        Screen::Startup                      => draw_startup(f, app, area),
        Screen::Menu                         => draw_menu(f, app, area),
        Screen::Settings                     => draw_settings(f, app, area),
        Screen::Stats                        => draw_stats(f, app, area),
        Screen::Teacher                      => draw_teacher(f, app, area),
        Screen::Cinematic                    => draw_cinematic(f, app, fx, area),
        Screen::Practice | Screen::Challenge => draw_session(f, app, fx, area),
        Screen::Experiment                   => draw_experiment(f, app, area),
    }
}
