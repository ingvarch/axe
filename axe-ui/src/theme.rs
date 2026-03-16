/// Visual theme definition for the IDE UI.
///
/// Contains colors for panels, borders, and the status bar.
/// Uses One Dark color scheme by default.
use ratatui::style::Color;

/// Holds all UI colors for rendering the IDE.
pub struct Theme {
    /// Main background color.
    pub background: Color,
    /// Default foreground/text color.
    pub foreground: Color,
    /// Border color for inactive panels.
    pub panel_border: Color,
    /// Border color for the active (focused) panel.
    pub panel_border_active: Color,
    /// Status bar background color.
    pub status_bar_bg: Color,
    /// Status bar foreground/text color.
    pub status_bar_fg: Color,
    /// Status bar hotkey label color (dimmed).
    pub status_bar_key: Color,
    /// Help overlay border color.
    pub overlay_border: Color,
    /// Help overlay background color.
    pub overlay_bg: Color,
    /// Border color used in resize mode.
    pub resize_border: Color,
    /// Background color for selected tree row.
    pub tree_selection_bg: Color,
    /// Background color for the line number gutter.
    pub gutter_bg: Color,
    /// Foreground color for non-active line numbers.
    pub line_number: Color,
    /// Foreground color for the active (cursor) line number.
    pub line_number_active: Color,
    /// Background color for the current cursor line.
    pub cursor_line_bg: Color,
    /// Background color for selected text.
    pub selection_bg: Color,
    /// Background color for search match highlights.
    pub search_match_bg: Color,
    /// Background color for the currently active search match.
    pub search_active_match_bg: Color,
    /// Foreground color for the currently active search match (dark for contrast).
    pub search_active_match_fg: Color,
    /// Background color for the tab bar row.
    pub tab_bar_bg: Color,
    /// Background color for the active tab.
    pub tab_active_bg: Color,
    /// Foreground color for the active tab.
    pub tab_active_fg: Color,
    /// Foreground color for inactive tabs.
    pub tab_inactive_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(40, 44, 52),                // #282c34
            foreground: Color::Rgb(171, 178, 191),             // #abb2bf
            panel_border: Color::Rgb(76, 82, 99),              // #4c5263
            panel_border_active: Color::Rgb(97, 175, 239),     // #61afef
            status_bar_bg: Color::Rgb(33, 37, 43),             // #21252b
            status_bar_fg: Color::Rgb(171, 178, 191),          // #abb2bf
            status_bar_key: Color::Rgb(130, 137, 151),         // #828997
            overlay_border: Color::Rgb(97, 175, 239),          // #61afef
            overlay_bg: Color::Rgb(40, 44, 52),                // #282c34
            resize_border: Color::Rgb(229, 192, 123),          // #e5c07b (yellow)
            tree_selection_bg: Color::Rgb(50, 55, 65),         // slightly lighter than bg
            gutter_bg: Color::Rgb(35, 39, 46),                 // #23272e — slightly darker than bg
            line_number: Color::Rgb(76, 82, 99),               // #4c5263 — dim
            line_number_active: Color::Rgb(171, 178, 191),     // #abb2bf — bright
            cursor_line_bg: Color::Rgb(45, 50, 60),            // subtle highlight
            selection_bg: Color::Rgb(67, 76, 94),              // #434c5e — medium blue-grey
            search_match_bg: Color::Rgb(60, 60, 30),           // subtle dark yellow tint
            search_active_match_bg: Color::Rgb(229, 192, 123), // #e5c07b — bright warm yellow
            search_active_match_fg: Color::Rgb(40, 44, 52),    // dark for contrast on bright bg
            tab_bar_bg: Color::Rgb(33, 37, 43),                // #21252b — same as status bar
            tab_active_bg: Color::Rgb(40, 44, 52),             // #282c34 — same as editor bg
            tab_active_fg: Color::Rgb(171, 178, 191),          // #abb2bf — bright foreground
            tab_inactive_fg: Color::Rgb(130, 137, 151),        // #828997 — dim foreground
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_distinct_border_colors() {
        let theme = Theme::default();
        assert_ne!(theme.panel_border, theme.panel_border_active);
    }

    #[test]
    fn default_theme_gutter_colors_are_set() {
        let theme = Theme::default();
        assert_ne!(
            theme.gutter_bg, theme.background,
            "gutter_bg should differ from background"
        );
        assert_ne!(theme.line_number, Color::Black);
        assert_ne!(theme.line_number_active, Color::Black);
    }

    #[test]
    fn cursor_line_bg_differs_from_background() {
        let theme = Theme::default();
        assert_ne!(
            theme.cursor_line_bg, theme.background,
            "cursor_line_bg should differ from background"
        );
    }

    #[test]
    fn selection_bg_differs_from_background() {
        let theme = Theme::default();
        assert_ne!(
            theme.selection_bg, theme.background,
            "selection_bg should differ from background"
        );
        assert_ne!(
            theme.selection_bg, theme.cursor_line_bg,
            "selection_bg should differ from cursor_line_bg"
        );
    }

    #[test]
    fn search_colors_differ_from_background() {
        let theme = Theme::default();
        assert_ne!(
            theme.search_match_bg, theme.background,
            "search_match_bg should differ from background"
        );
        assert_ne!(
            theme.search_active_match_bg, theme.background,
            "search_active_match_bg should differ from background"
        );
        assert_ne!(
            theme.search_match_bg, theme.search_active_match_bg,
            "search match colors should differ from each other"
        );
    }

    #[test]
    fn tab_colors_are_set() {
        let theme = Theme::default();
        assert_ne!(
            theme.tab_active_bg, theme.tab_bar_bg,
            "active tab should stand out from bar"
        );
        assert_ne!(
            theme.tab_active_fg, theme.tab_inactive_fg,
            "active/inactive tab text should differ"
        );
    }

    #[test]
    fn default_theme_has_non_black_colors() {
        let theme = Theme::default();
        let black = Color::Black;
        assert_ne!(theme.background, black);
        assert_ne!(theme.foreground, black);
        assert_ne!(theme.panel_border, black);
        assert_ne!(theme.panel_border_active, black);
        assert_ne!(theme.status_bar_bg, black);
        assert_ne!(theme.status_bar_fg, black);
    }
}
