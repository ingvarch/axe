//! Layout configuration for the three-panel IDE layout.
//!
//! Controls the relative sizing of the file tree, editor, and terminal panels.

/// Manages panel size percentages for the IDE layout.
pub struct LayoutManager {
    /// Width of the file tree panel as a percentage of total width.
    pub tree_width_pct: u16,
    /// Height of the editor panel as a percentage of the right-side area.
    pub editor_height_pct: u16,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            tree_width_pct: 20,
            editor_height_pct: 70,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_twenty_percent_tree_width() {
        let layout = LayoutManager::default();
        assert_eq!(layout.tree_width_pct, 20);
    }

    #[test]
    fn default_layout_has_seventy_percent_editor_height() {
        let layout = LayoutManager::default();
        assert_eq!(layout.editor_height_pct, 70);
    }
}
