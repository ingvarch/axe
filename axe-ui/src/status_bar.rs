use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use axe_core::AppState;
use axe_editor::diagnostic::{diagnostic_counts, diagnostics_for_line};

use crate::editor_panel::diagnostic_color;
use crate::theme::Theme;

/// Builds the left section of the status bar: mode badge, filename, modified indicator.
pub(crate) fn build_status_left<'a>(app: &AppState, theme: &Theme) -> Vec<Span<'a>> {
    let text_style = Style::default().fg(theme.status_bar_fg);
    let mode_style = Style::default()
        .bg(theme.status_bar_mode_bg)
        .fg(theme.status_bar_mode_fg)
        .add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span<'a>> = Vec::new();

    // Mode badge (RESIZE or ZOOM).
    if app.resize_mode.active {
        spans.push(Span::styled(" RESIZE ", mode_style));
        spans.push(Span::styled(" ", text_style));
    } else if app.zoomed_panel.is_some() {
        spans.push(Span::styled(" ZOOM ", mode_style));
        spans.push(Span::styled(" ", text_style));
    }

    // Filename or app name when no buffer is open.
    if let Some(buffer) = app.buffer_manager.active_buffer() {
        let name = buffer.file_name().unwrap_or("[untitled]");
        spans.push(Span::styled(format!(" {name}"), text_style));
        if buffer.modified {
            spans.push(Span::styled(
                " [+]",
                Style::default().fg(theme.status_bar_key),
            ));
        }
    } else {
        let version = &app.build_version;
        let label = if version.is_empty() {
            format!(" Axe v{}", env!("CARGO_PKG_VERSION"))
        } else {
            format!(" Axe {version}")
        };
        spans.push(Span::styled(label, text_style));
    }

    spans
}

/// Builds the center section of the status bar: notification or cursor-line diagnostic.
pub(crate) fn build_status_center<'a>(app: &AppState, theme: &Theme) -> Vec<Span<'a>> {
    if let Some((ref msg, _)) = app.status_message {
        return vec![Span::styled(
            msg.clone(),
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        )];
    }

    if let Some(buffer) = app.buffer_manager.active_buffer() {
        if let Some(diag) = diagnostics_for_line(buffer.diagnostics(), buffer.cursor.row).next() {
            let color = diagnostic_color(diag.severity, theme);
            return vec![Span::styled(
                diag.message.clone(),
                Style::default().fg(color),
            )];
        }
    }

    Vec::new()
}

/// Builds the right section of the status bar: file type, encoding, line ending, cursor,
/// git branch, and diagnostic counts.
pub(crate) fn build_status_right<'a>(app: &AppState, theme: &Theme) -> Vec<Span<'a>> {
    let text_style = Style::default().fg(theme.status_bar_fg);
    let sep_style = Style::default().fg(theme.status_bar_key);
    let sep = || Span::styled(" | ", sep_style);

    let Some(buffer) = app.buffer_manager.active_buffer() else {
        return Vec::new();
    };

    let mut spans: Vec<Span<'a>> = Vec::new();

    // File type.
    spans.push(Span::styled(buffer.file_type().to_string(), text_style));

    // Encoding (always UTF-8 — Rope only supports UTF-8).
    spans.push(sep());
    spans.push(Span::styled("UTF-8", text_style));

    // Line ending.
    spans.push(sep());
    spans.push(Span::styled(
        buffer.line_ending().as_str().to_string(),
        text_style,
    ));

    // Cursor position.
    spans.push(sep());
    spans.push(Span::styled(
        format!(
            "Ln {}, Col {}",
            buffer.cursor.row + 1,
            buffer.cursor.col + 1
        ),
        text_style,
    ));

    // Git branch.
    if let Some(ref branch) = app.git_branch {
        spans.push(sep());
        spans.push(Span::styled(format!("\u{2387} {branch}"), text_style));
    }

    // Diagnostic counts (only show non-zero).
    let (errors, warnings, _infos, _hints) = diagnostic_counts(buffer.diagnostics());
    if errors > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            format!("E:{errors}"),
            Style::default().fg(theme.diagnostic_error),
        ));
    }
    if warnings > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            format!("W:{warnings}"),
            Style::default().fg(theme.diagnostic_warning),
        ));
    }

    spans.push(Span::styled(" ", text_style));
    spans
}

/// Builds the status bar line with left/center/right layout.
pub(crate) fn build_status_bar<'a>(app: &AppState, theme: &Theme, width: u16) -> Line<'a> {
    let left = build_status_left(app, theme);
    let center = build_status_center(app, theme);
    let right = build_status_right(app, theme);

    let left_w: usize = left.iter().map(|s| s.width()).sum();
    let center_w: usize = center.iter().map(|s| s.width()).sum();
    let right_w: usize = right.iter().map(|s| s.width()).sum();
    let total = width as usize;

    let bg_style = Style::default().bg(theme.status_bar_bg);
    let mut spans = left;

    let gap = total.saturating_sub(left_w + right_w);
    if center_w > 0 && gap > center_w {
        let left_pad = (gap - center_w) / 2;
        let right_pad = gap - center_w - left_pad;
        spans.push(Span::styled(" ".repeat(left_pad), bg_style));
        spans.extend(center);
        spans.push(Span::styled(" ".repeat(right_pad), bg_style));
    } else {
        spans.push(Span::styled(" ".repeat(gap), bg_style));
    }

    spans.extend(right);
    Line::from(spans)
}
