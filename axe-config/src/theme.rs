use std::collections::HashMap;
use std::path::Path;

use ratatui::style::Color;
use serde::Deserialize;

/// Bundled One Dark theme TOML.
const BUNDLED_AXE_DARK: &str = include_str!("../../themes/axe-dark.toml");
/// Bundled light theme TOML.
const BUNDLED_AXE_LIGHT: &str = include_str!("../../themes/axe-light.toml");

/// Top-level theme file structure matching the TOML layout.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThemeFile {
    #[serde(default)]
    pub base: BaseColors,
    #[serde(default)]
    pub ui: UiColors,
    #[serde(default)]
    pub gutter: GutterColors,
    #[serde(default)]
    pub editor: EditorColors,
    #[serde(default)]
    pub syntax: HashMap<String, SyntaxStyle>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BaseColors {
    pub background: Option<String>,
    pub foreground: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UiColors {
    pub panel_border: Option<String>,
    pub panel_border_active: Option<String>,
    pub status_bar_bg: Option<String>,
    pub status_bar_fg: Option<String>,
    pub status_bar_key: Option<String>,
    pub overlay_border: Option<String>,
    pub overlay_bg: Option<String>,
    pub resize_border: Option<String>,
    pub tree_selection_bg: Option<String>,
    pub tab_bar_bg: Option<String>,
    pub tab_active_bg: Option<String>,
    pub tab_active_fg: Option<String>,
    pub tab_inactive_fg: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GutterColors {
    pub bg: Option<String>,
    pub line_number: Option<String>,
    pub line_number_active: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EditorColors {
    pub cursor_line_bg: Option<String>,
    pub selection_bg: Option<String>,
    pub search_match_bg: Option<String>,
    pub search_active_match_bg: Option<String>,
    pub search_active_match_fg: Option<String>,
    pub diagnostic_error: Option<String>,
    pub diagnostic_warning: Option<String>,
    pub diagnostic_info: Option<String>,
    pub diagnostic_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SyntaxStyle {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
}

/// Parses a hex color string like "#rrggbb" into a ratatui Color.
///
/// Returns `None` for invalid formats.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Loads a theme by name.
///
/// Search order:
/// 1. `~/.config/axe/themes/{name}.toml` (user themes)
/// 2. Bundled themes (axe-dark, axe-light)
///
/// Returns `None` if the theme is not found.
pub fn load_theme(name: &str) -> Option<ThemeFile> {
    // Check user theme directory first (~/.config/axe/themes/).
    if let Some(home) = dirs::home_dir() {
        let user_theme_path = home
            .join(".config")
            .join("axe")
            .join("themes")
            .join(format!("{name}.toml"));
        if let Some(theme) = load_theme_from_path(&user_theme_path) {
            return Some(theme);
        }
    }

    // Fall back to bundled themes.
    load_bundled_theme(name)
}

/// Loads a theme from a specific file path.
fn load_theme_from_path(path: &Path) -> Option<ThemeFile> {
    let content = std::fs::read_to_string(path).ok()?;
    match toml::from_str(&content) {
        Ok(theme) => Some(theme),
        Err(e) => {
            log::warn!("Failed to parse theme file {}: {e}", path.display());
            None
        }
    }
}

/// Loads a bundled theme by name.
fn load_bundled_theme(name: &str) -> Option<ThemeFile> {
    let content = match name {
        "axe-dark" => Some(BUNDLED_AXE_DARK),
        "axe-light" => Some(BUNDLED_AXE_LIGHT),
        _ => None,
    }?;

    match toml::from_str(content) {
        Ok(theme) => Some(theme),
        Err(e) => {
            log::warn!("Failed to parse bundled theme '{name}': {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#c678dd"), Some(Color::Rgb(198, 120, 221)));
        assert_eq!(parse_hex_color("#000000"), Some(Color::Rgb(0, 0, 0)));
        assert_eq!(parse_hex_color("#ffffff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(parse_hex_color("#282c34"), Some(Color::Rgb(40, 44, 52)));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#"), None);
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
        assert_eq!(parse_hex_color("282c34"), None);
        assert_eq!(parse_hex_color("#282c3"), None);
        assert_eq!(parse_hex_color("#282c34ff"), None);
    }

    #[test]
    fn load_bundled_axe_dark() {
        let theme = load_theme("axe-dark");
        assert!(theme.is_some(), "axe-dark should be a bundled theme");
        let theme = theme.expect("already checked is_some");
        assert!(theme.base.background.is_some());
        assert!(theme.base.foreground.is_some());
        assert!(!theme.syntax.is_empty(), "theme should have syntax styles");
        assert!(theme.syntax.contains_key("keyword"));
        assert!(theme.syntax.contains_key("string"));
    }

    #[test]
    fn load_bundled_axe_light() {
        let theme = load_theme("axe-light");
        assert!(theme.is_some(), "axe-light should be a bundled theme");
        let theme = theme.expect("already checked is_some");
        assert!(theme.base.background.is_some());
        assert!(theme.base.foreground.is_some());
    }

    #[test]
    fn load_unknown_theme_returns_none() {
        assert!(load_theme("nonexistent-theme-xyz").is_none());
    }

    #[test]
    fn theme_file_deserialize_empty_toml() {
        let theme: ThemeFile = toml::from_str("").expect("empty TOML should parse");
        assert!(theme.base.background.is_none());
        assert!(theme.syntax.is_empty());
    }

    #[test]
    fn theme_file_deserialize_partial() {
        let toml_str = r##"
[base]
background = "#112233"

[syntax]
keyword = { fg = "#aabbcc" }
"##;
        let theme: ThemeFile = toml::from_str(toml_str).expect("partial TOML should parse");
        assert_eq!(theme.base.background.as_deref(), Some("#112233"));
        assert!(theme.base.foreground.is_none());
        assert_eq!(
            theme.syntax.get("keyword").and_then(|s| s.fg.as_deref()),
            Some("#aabbcc")
        );
    }

    #[test]
    fn load_theme_from_path_nonexistent() {
        let result = load_theme_from_path(Path::new("/tmp/nonexistent_axe_theme_test.toml"));
        assert!(result.is_none());
    }

    #[test]
    fn load_theme_from_path_valid_file() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let path = dir.path().join("test-theme.toml");
        std::fs::write(
            &path,
            r##"
[base]
background = "#000000"
foreground = "#ffffff"
"##,
        )
        .expect("should write theme file");
        let theme = load_theme_from_path(&path);
        assert!(theme.is_some());
        assert_eq!(
            theme
                .expect("already checked is_some")
                .base
                .background
                .as_deref(),
            Some("#000000")
        );
    }

    #[test]
    fn load_theme_from_path_invalid_toml() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let path = dir.path().join("bad-theme.toml");
        std::fs::write(&path, "this is not valid toml [[[").expect("should write file");
        let theme = load_theme_from_path(&path);
        assert!(theme.is_none());
    }

    #[test]
    fn syntax_style_with_modifiers() {
        let toml_str = r##"
[syntax]
comment = { fg = "#5c6370", italic = true }
keyword = { fg = "#c678dd", bold = true }
"##;
        let theme: ThemeFile = toml::from_str(toml_str).expect("should parse syntax styles");
        let comment = theme.syntax.get("comment").expect("comment should exist");
        assert_eq!(comment.italic, Some(true));
        assert_eq!(comment.bold, None);
        let keyword = theme.syntax.get("keyword").expect("keyword should exist");
        assert_eq!(keyword.bold, Some(true));
        assert_eq!(keyword.italic, None);
    }
}
