// Event handling utilities for the TUI
// Currently keyboard events are handled directly in mod.rs
// This module is reserved for future event handling extensions

use crossterm::event::KeyCode;

/// Convert key code to description for help display
pub fn key_description(key: KeyCode) -> &'static str {
    match key {
        KeyCode::Tab => "Tab",
        KeyCode::BackTab => "Shift+Tab",
        KeyCode::Left => "Left",
        KeyCode::Right => "Right",
        KeyCode::Up => "Up",
        KeyCode::Down => "Down",
        KeyCode::Enter => "Enter",
        KeyCode::Esc => "Esc",
        KeyCode::Char('q') => "q",
        KeyCode::Char('?') => "?",
        KeyCode::Char('s') => "s",
        KeyCode::Char('r') => "r",
        KeyCode::Char('j') => "j",
        KeyCode::Char('k') => "k",
        _ => "Unknown",
    }
}
