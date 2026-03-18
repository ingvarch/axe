/// Visual theme definition for the IDE UI.
///
/// Contains colors for panels, borders, status bar, and syntax highlighting.
/// Uses One Dark color scheme by default.
use ratatui::style::Color;

use axe_config::theme::{parse_hex_color, ThemeFile};
use axe_editor::HighlightKind;

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
    // --- Syntax highlighting colors ---
    /// Keyword color (purple).
    pub syntax_keyword: Color,
    /// String literal color (green).
    pub syntax_string: Color,
    /// Comment color (grey).
    pub syntax_comment: Color,
    /// Function name color (blue).
    pub syntax_function: Color,
    /// Type name color (yellow).
    pub syntax_type: Color,
    /// Variable color (red).
    pub syntax_variable: Color,
    /// Constant color (orange).
    pub syntax_constant: Color,
    /// Number literal color (orange).
    pub syntax_number: Color,
    /// Operator color (cyan).
    pub syntax_operator: Color,
    /// Punctuation color (foreground).
    pub syntax_punctuation: Color,
    /// Property/field color (red).
    pub syntax_property: Color,
    /// Attribute color (yellow).
    pub syntax_attribute: Color,
    /// Tag color (red).
    pub syntax_tag: Color,
    /// Escape sequence color (cyan).
    pub syntax_escape: Color,
    /// Builtin function/variable color (cyan).
    pub syntax_builtin: Color,
    // --- Git diff gutter colors ---
    /// Color for added lines in the gutter (green).
    pub diff_added: Color,
    /// Color for modified lines in the gutter (blue).
    pub diff_modified: Color,
    /// Color for deleted lines in the gutter (red).
    pub diff_deleted: Color,
    /// Color for modified file names in the tree panel (orange).
    pub tree_modified_fg: Color,
    /// Background color for the mode indicator badge (e.g., RESIZE, ZOOM).
    pub status_bar_mode_bg: Color,
    /// Foreground color for the mode indicator badge text.
    pub status_bar_mode_fg: Color,
    // --- Diagnostic colors ---
    /// Diagnostic error color (red).
    pub diagnostic_error: Color,
    /// Diagnostic warning color (yellow).
    pub diagnostic_warning: Color,
    /// Diagnostic info color (blue).
    pub diagnostic_info: Color,
    /// Diagnostic hint color (grey).
    pub diagnostic_hint: Color,
}

impl Theme {
    /// Builds a Theme from a parsed theme file, using `Default` for any missing colors.
    pub fn from_theme_file(tf: &ThemeFile) -> Self {
        let defaults = Self::default();

        /// Resolves an optional hex string to a Color, falling back to a default.
        fn color_or(hex: &Option<String>, fallback: Color) -> Color {
            hex.as_deref()
                .and_then(|h| {
                    let c = parse_hex_color(h);
                    if c.is_none() {
                        log::warn!("Invalid hex color: {h}");
                    }
                    c
                })
                .unwrap_or(fallback)
        }

        /// Resolves a syntax entry's foreground color.
        fn syntax_fg(tf: &ThemeFile, key: &str, fallback: Color) -> Color {
            tf.syntax
                .get(key)
                .and_then(|s| s.fg.as_deref())
                .and_then(parse_hex_color)
                .unwrap_or(fallback)
        }

        Self {
            background: color_or(&tf.base.background, defaults.background),
            foreground: color_or(&tf.base.foreground, defaults.foreground),
            panel_border: color_or(&tf.ui.panel_border, defaults.panel_border),
            panel_border_active: color_or(&tf.ui.panel_border_active, defaults.panel_border_active),
            status_bar_bg: color_or(&tf.ui.status_bar_bg, defaults.status_bar_bg),
            status_bar_fg: color_or(&tf.ui.status_bar_fg, defaults.status_bar_fg),
            status_bar_key: color_or(&tf.ui.status_bar_key, defaults.status_bar_key),
            overlay_border: color_or(&tf.ui.overlay_border, defaults.overlay_border),
            overlay_bg: color_or(&tf.ui.overlay_bg, defaults.overlay_bg),
            resize_border: color_or(&tf.ui.resize_border, defaults.resize_border),
            tree_selection_bg: color_or(&tf.ui.tree_selection_bg, defaults.tree_selection_bg),
            gutter_bg: color_or(&tf.gutter.bg, defaults.gutter_bg),
            line_number: color_or(&tf.gutter.line_number, defaults.line_number),
            line_number_active: color_or(
                &tf.gutter.line_number_active,
                defaults.line_number_active,
            ),
            cursor_line_bg: color_or(&tf.editor.cursor_line_bg, defaults.cursor_line_bg),
            selection_bg: color_or(&tf.editor.selection_bg, defaults.selection_bg),
            search_match_bg: color_or(&tf.editor.search_match_bg, defaults.search_match_bg),
            search_active_match_bg: color_or(
                &tf.editor.search_active_match_bg,
                defaults.search_active_match_bg,
            ),
            search_active_match_fg: color_or(
                &tf.editor.search_active_match_fg,
                defaults.search_active_match_fg,
            ),
            tab_bar_bg: color_or(&tf.ui.tab_bar_bg, defaults.tab_bar_bg),
            tab_active_bg: color_or(&tf.ui.tab_active_bg, defaults.tab_active_bg),
            tab_active_fg: color_or(&tf.ui.tab_active_fg, defaults.tab_active_fg),
            tab_inactive_fg: color_or(&tf.ui.tab_inactive_fg, defaults.tab_inactive_fg),
            syntax_keyword: syntax_fg(tf, "keyword", defaults.syntax_keyword),
            syntax_string: syntax_fg(tf, "string", defaults.syntax_string),
            syntax_comment: syntax_fg(tf, "comment", defaults.syntax_comment),
            syntax_function: syntax_fg(tf, "function", defaults.syntax_function),
            syntax_type: syntax_fg(tf, "type", defaults.syntax_type),
            syntax_variable: syntax_fg(tf, "variable", defaults.syntax_variable),
            syntax_constant: syntax_fg(tf, "constant", defaults.syntax_constant),
            syntax_number: syntax_fg(tf, "number", defaults.syntax_number),
            syntax_operator: syntax_fg(tf, "operator", defaults.syntax_operator),
            syntax_punctuation: syntax_fg(tf, "punctuation", defaults.syntax_punctuation),
            syntax_property: syntax_fg(tf, "property", defaults.syntax_property),
            syntax_attribute: syntax_fg(tf, "attribute", defaults.syntax_attribute),
            syntax_tag: syntax_fg(tf, "tag", defaults.syntax_tag),
            syntax_escape: syntax_fg(tf, "escape", defaults.syntax_escape),
            syntax_builtin: syntax_fg(tf, "builtin", defaults.syntax_builtin),
            diff_added: color_or(&tf.gutter.diff_added, defaults.diff_added),
            diff_modified: color_or(&tf.gutter.diff_modified, defaults.diff_modified),
            diff_deleted: color_or(&tf.gutter.diff_deleted, defaults.diff_deleted),
            tree_modified_fg: color_or(&tf.ui.tree_modified_fg, defaults.tree_modified_fg),
            status_bar_mode_bg: color_or(&tf.ui.status_bar_mode_bg, defaults.status_bar_mode_bg),
            status_bar_mode_fg: color_or(&tf.ui.status_bar_mode_fg, defaults.status_bar_mode_fg),
            diagnostic_error: color_or(&tf.editor.diagnostic_error, defaults.diagnostic_error),
            diagnostic_warning: color_or(
                &tf.editor.diagnostic_warning,
                defaults.diagnostic_warning,
            ),
            diagnostic_info: color_or(&tf.editor.diagnostic_info, defaults.diagnostic_info),
            diagnostic_hint: color_or(&tf.editor.diagnostic_hint, defaults.diagnostic_hint),
        }
    }

    /// Returns the foreground color for a syntax highlight kind.
    pub fn syntax_color(&self, kind: HighlightKind) -> Color {
        match kind {
            HighlightKind::Keyword => self.syntax_keyword,
            HighlightKind::String => self.syntax_string,
            HighlightKind::Comment => self.syntax_comment,
            HighlightKind::Function => self.syntax_function,
            HighlightKind::Type => self.syntax_type,
            HighlightKind::Variable => self.syntax_variable,
            HighlightKind::Constant => self.syntax_constant,
            HighlightKind::Number => self.syntax_number,
            HighlightKind::Operator => self.syntax_operator,
            HighlightKind::Punctuation => self.syntax_punctuation,
            HighlightKind::Property => self.syntax_property,
            HighlightKind::Attribute => self.syntax_attribute,
            HighlightKind::Tag => self.syntax_tag,
            HighlightKind::Escape => self.syntax_escape,
            HighlightKind::Builtin => self.syntax_builtin,
        }
    }
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
            // One Dark syntax colors
            syntax_keyword: Color::Rgb(198, 120, 221), // #c678dd — purple
            syntax_string: Color::Rgb(152, 195, 121),  // #98c379 — green
            syntax_comment: Color::Rgb(92, 99, 112),   // #5c6370 — grey
            syntax_function: Color::Rgb(97, 175, 239), // #61afef — blue
            syntax_type: Color::Rgb(229, 192, 123),    // #e5c07b — yellow
            syntax_variable: Color::Rgb(224, 108, 117), // #e06c75 — red
            syntax_constant: Color::Rgb(209, 154, 102), // #d19a66 — orange
            syntax_number: Color::Rgb(209, 154, 102),  // #d19a66 — orange
            syntax_operator: Color::Rgb(86, 182, 194), // #56b6c2 — cyan
            syntax_punctuation: Color::Rgb(171, 178, 191), // #abb2bf — foreground
            syntax_property: Color::Rgb(224, 108, 117), // #e06c75 — red
            syntax_attribute: Color::Rgb(229, 192, 123), // #e5c07b — yellow
            syntax_tag: Color::Rgb(224, 108, 117),     // #e06c75 — red
            syntax_escape: Color::Rgb(86, 182, 194),   // #56b6c2 — cyan
            syntax_builtin: Color::Rgb(86, 182, 194),  // #56b6c2 — cyan
            // Git diff gutter colors
            diff_added: Color::Rgb(152, 195, 121), // #98c379 — green
            diff_modified: Color::Rgb(97, 175, 239), // #61afef — blue
            diff_deleted: Color::Rgb(224, 108, 117), // #e06c75 — red
            tree_modified_fg: Color::Rgb(209, 154, 102), // #d19a66 — orange
            status_bar_mode_bg: Color::Rgb(97, 175, 239), // #61afef — blue
            status_bar_mode_fg: Color::Rgb(40, 44, 52), // #282c34 — dark
            // Diagnostic colors
            diagnostic_error: Color::Rgb(224, 108, 117), // #e06c75 — red
            diagnostic_warning: Color::Rgb(229, 192, 123), // #e5c07b — yellow
            diagnostic_info: Color::Rgb(97, 175, 239),   // #61afef — blue
            diagnostic_hint: Color::Rgb(130, 137, 151),  // #828997 — grey
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

    #[test]
    fn syntax_colors_differ_from_background() {
        let theme = Theme::default();
        assert_ne!(theme.syntax_keyword, theme.background);
        assert_ne!(theme.syntax_string, theme.background);
        assert_ne!(theme.syntax_comment, theme.background);
        assert_ne!(theme.syntax_function, theme.background);
        assert_ne!(theme.syntax_type, theme.background);
        assert_ne!(theme.syntax_number, theme.background);
    }

    #[test]
    fn syntax_color_maps_all_kinds() {
        let theme = Theme::default();
        let kinds = [
            HighlightKind::Keyword,
            HighlightKind::String,
            HighlightKind::Comment,
            HighlightKind::Function,
            HighlightKind::Type,
            HighlightKind::Variable,
            HighlightKind::Constant,
            HighlightKind::Number,
            HighlightKind::Operator,
            HighlightKind::Punctuation,
            HighlightKind::Property,
            HighlightKind::Attribute,
            HighlightKind::Tag,
            HighlightKind::Escape,
            HighlightKind::Builtin,
        ];
        for kind in kinds {
            let color = theme.syntax_color(kind);
            assert_ne!(
                color,
                Color::Black,
                "syntax_color({kind:?}) should not be black"
            );
        }
    }

    #[test]
    fn syntax_keyword_differs_from_string() {
        let theme = Theme::default();
        assert_ne!(
            theme.syntax_keyword, theme.syntax_string,
            "keyword and string should have different colors"
        );
    }

    #[test]
    fn from_empty_theme_file_equals_default() {
        let tf = ThemeFile::default();
        let theme = Theme::from_theme_file(&tf);
        let defaults = Theme::default();
        assert_eq!(theme.background, defaults.background);
        assert_eq!(theme.foreground, defaults.foreground);
        assert_eq!(theme.syntax_keyword, defaults.syntax_keyword);
    }

    #[test]
    fn from_theme_file_overrides_colors() {
        let mut tf = ThemeFile::default();
        tf.base.background = Some("#112233".to_string());
        tf.base.foreground = Some("#aabbcc".to_string());
        let theme = Theme::from_theme_file(&tf);
        assert_eq!(theme.background, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(theme.foreground, Color::Rgb(0xaa, 0xbb, 0xcc));
        let defaults = Theme::default();
        assert_eq!(theme.panel_border, defaults.panel_border);
    }

    #[test]
    fn from_theme_file_invalid_hex_falls_back() {
        let mut tf = ThemeFile::default();
        tf.base.background = Some("not-a-color".to_string());
        let theme = Theme::from_theme_file(&tf);
        let defaults = Theme::default();
        assert_eq!(theme.background, defaults.background);
    }

    #[test]
    fn from_theme_file_syntax_colors() {
        use axe_config::theme::SyntaxStyle;
        use std::collections::HashMap;

        let mut syntax = HashMap::new();
        syntax.insert(
            "keyword".to_string(),
            SyntaxStyle {
                fg: Some("#ff0000".to_string()),
                bg: None,
                bold: None,
                italic: None,
            },
        );
        let tf = ThemeFile {
            syntax,
            ..ThemeFile::default()
        };
        let theme = Theme::from_theme_file(&tf);
        assert_eq!(theme.syntax_keyword, Color::Rgb(255, 0, 0));
        let defaults = Theme::default();
        assert_eq!(theme.syntax_string, defaults.syntax_string);
    }

    #[test]
    fn from_bundled_axe_dark_matches_default() {
        let tf = axe_config::theme::load_theme("axe-dark").expect("bundled theme should load");
        let theme = Theme::from_theme_file(&tf);
        let defaults = Theme::default();
        assert_eq!(theme.background, defaults.background);
        assert_eq!(theme.foreground, defaults.foreground);
        assert_eq!(theme.syntax_keyword, defaults.syntax_keyword);
        assert_eq!(theme.syntax_string, defaults.syntax_string);
    }

    #[test]
    fn default_theme_has_diagnostic_colors() {
        let theme = Theme::default();
        assert_ne!(theme.diagnostic_error, Color::Black);
        assert_ne!(theme.diagnostic_warning, Color::Black);
        assert_ne!(theme.diagnostic_info, Color::Black);
        assert_ne!(theme.diagnostic_hint, Color::Black);
    }

    #[test]
    fn diagnostic_colors_differ() {
        let theme = Theme::default();
        assert_ne!(theme.diagnostic_error, theme.diagnostic_warning);
        assert_ne!(theme.diagnostic_warning, theme.diagnostic_info);
        assert_ne!(theme.diagnostic_info, theme.diagnostic_hint);
    }

    #[test]
    fn from_bundled_axe_light_differs_from_default() {
        let tf = axe_config::theme::load_theme("axe-light").expect("bundled theme should load");
        let theme = Theme::from_theme_file(&tf);
        let defaults = Theme::default();
        assert_ne!(
            theme.background, defaults.background,
            "light theme should have different background"
        );
    }
}
