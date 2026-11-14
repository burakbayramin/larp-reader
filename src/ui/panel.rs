//! The side panel: the keys, and the openable files in the current file's folder.
//!
//! Everything here works from the current file's absolute path (`current`);
//! the panel's open/closed state stays with `App`.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::text::{Line as TLine, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use super::style::style;
use crate::loaders::{suffix, supported_extensions};

/// One per digit key.
pub const LISTED_INPUTS: usize = 9;
const PANEL_WIDTH: u16 = 38;
const KEYS: &[(&str, &str)] = &[
    ("space", "page down"),
    ("u, pgup", "page up"),
    ("j / k", "line down / up"),
    ("] / [", "next / prev section"),
    ("/", "find"),
    ("f / F", "find next / prev"),
    ("a", "auto scroll"),
    ("+ / -", "reading rate"),
    ("m / M", "next / prev view"),
    ("l / L", "next / prev language"),
    ("n / N", "more / less filler"),
    ("o / O", "next / prev input"),
    ("1-9", "open listed input"),
    ("b", "mark this place"),
    ("' / \"", "next / prev mark"),
    (":", "config"),
    ("`", "hide / unhide"),
    ("esc", "this panel"),
    ("q", "quit"),
];

/// The openable files next to `current`, in name order.
pub fn inputs(current: Option<&str>) -> Vec<PathBuf> {
    let Some(current) = current.map(PathBuf::from) else {
        return vec![];
    };
    let known = supported_extensions();
    let mut found: Vec<PathBuf> = current
        .parent()
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && known.contains(&suffix(p).as_str()))
                .collect()
        })
        .unwrap_or_default();
    if !found.contains(&current) {
        found.push(current);
    }
    found.sort_by_key(|f| {
        f.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    found
}

/// The file `step` places from `current` in the folder, wrapping around; none
/// unless there are at least two.
pub fn cycle_target(current: Option<&str>, step: isize) -> Option<PathBuf> {
    let files = inputs(current);
    if files.len() < 2 {
        return None;
    }
    let current = current.map(PathBuf::from);
    let i = files
        .iter()
        .position(|f| Some(f) == current.as_ref())
        .unwrap_or(0) as isize;
    Some(files[(i + step).rem_euclid(files.len() as isize) as usize].clone())
}

/// The file listed under digit `n`, unless there is none or it is `current`.
pub fn listed_target(current: Option<&str>, n: usize) -> Option<PathBuf> {
    let files = inputs(current);
    if n > files.len().min(LISTED_INPUTS) {
        return None;
    }
    let target = files[n - 1].clone();
    (Some(target.to_string_lossy().as_ref()) != current).then_some(target)
}

/// The side panel's text: the keys, then the folder's openable files.
pub fn panel_text(current: Option<&str>) -> Vec<TLine<'static>> {
    let (bold, key_style, text_style, dim) = (
        style("bold #d4d4d4"),
        style("#dcdcaa"),
        style("#cccccc"),
        style("#858585"),
    );
    let mut out = vec![TLine::styled("keys", bold)];
    for (k, what) in KEYS {
        out.push(TLine::from(vec![
            Span::styled(format!("{k:<9}"), key_style),
            Span::styled(*what, text_style),
        ]));
    }
    let files = inputs(current);
    if !files.is_empty() {
        out.push(TLine::raw(""));
        out.push(TLine::styled("inputs", bold));
        let current = current.map(PathBuf::from);
        for (n, f) in files.iter().take(LISTED_INPUTS).enumerate() {
            let here = Some(f) == current.as_ref();
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mark = if here {
                ">".to_string()
            } else {
                (n + 1).to_string()
            };
            out.push(TLine::from(vec![
                Span::styled(format!("{mark} "), key_style),
                Span::styled(name, if here { bold } else { text_style }),
            ]));
        }
        if files.len() > LISTED_INPUTS {
            out.push(TLine::styled(
                format!("  +{} more (o / O)", files.len() - LISTED_INPUTS),
                dim,
            ));
        }
    }
    out
}

/// Draw the panel over the right edge of the screen.
pub fn draw_panel(frame: &mut Frame, current: Option<&str>) {
    let area = frame.area();
    let width = PANEL_WIDTH.min(area.width);
    let rect = ratatui::layout::Rect {
        x: area.x + area.width - width,
        y: area.y,
        width,
        height: area.height,
    };
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(style("#3c3c3c"))
        .padding(Padding::new(2, 2, 1, 1))
        .style(style("#cccccc on #252526"));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(panel_text(current)).block(block), rect);
}
