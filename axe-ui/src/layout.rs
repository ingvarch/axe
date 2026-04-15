//! Layout configuration for the three-panel IDE layout.
//!
//! Controls the relative sizing of the file tree, editor, and terminal panels.

use axe_core::SplitOrientation;
use ratatui::layout::Rect;

/// Divides `area` into `count` equally-sized sub-rectangles along the
/// given orientation.
///
/// Any leftover pixels from integer division are distributed to the
/// first N rectangles so the sum always matches `area`. Returns an
/// empty vector when `count == 0`.
pub fn split_rects(area: Rect, count: usize, orientation: SplitOrientation) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let mut rects = Vec::with_capacity(count);
    match orientation {
        SplitOrientation::Horizontal => {
            let total = area.width as usize;
            let each = total / count;
            let remainder = total % count;
            let mut x = area.x;
            for i in 0..count {
                let width = (each + if i < remainder { 1 } else { 0 }) as u16;
                rects.push(Rect {
                    x,
                    y: area.y,
                    width,
                    height: area.height,
                });
                x += width;
            }
        }
        SplitOrientation::Vertical => {
            let total = area.height as usize;
            let each = total / count;
            let remainder = total % count;
            let mut y = area.y;
            for i in 0..count {
                let height = (each + if i < remainder { 1 } else { 0 }) as u16;
                rects.push(Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height,
                });
                y += height;
            }
        }
    }
    rects
}

/// Default width of the file tree panel as a percentage of total width.
const DEFAULT_TREE_WIDTH_PCT: u16 = 20;
/// Default height of the editor panel as a percentage of the right-side area.
const DEFAULT_EDITOR_HEIGHT_PCT: u16 = 70;

/// Manages panel size percentages for the IDE layout.
pub struct LayoutManager {
    /// Width of the file tree panel as a percentage of total width.
    pub tree_width_pct: u16,
    /// Height of the editor panel as a percentage of the right-side area.
    pub editor_height_pct: u16,
    /// Whether the file tree panel is visible.
    pub show_tree: bool,
    /// Whether the terminal panel is visible.
    pub show_terminal: bool,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            tree_width_pct: DEFAULT_TREE_WIDTH_PCT,
            editor_height_pct: DEFAULT_EDITOR_HEIGHT_PCT,
            show_tree: true,
            show_terminal: true,
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

    #[test]
    fn default_layout_shows_tree() {
        let layout = LayoutManager::default();
        assert!(layout.show_tree);
    }

    #[test]
    fn default_layout_shows_terminal() {
        let layout = LayoutManager::default();
        assert!(layout.show_terminal);
    }

    #[test]
    fn split_rects_single_returns_full_area() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = split_rects(area, 1, SplitOrientation::Horizontal);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], area);
    }

    #[test]
    fn split_rects_horizontal_divides_width() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = split_rects(area, 2, SplitOrientation::Horizontal);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[0].width, 40);
        assert_eq!(rects[1].x, 40);
        assert_eq!(rects[1].width, 40);
        // Both rects span the full height.
        assert_eq!(rects[0].height, 24);
        assert_eq!(rects[1].height, 24);
    }

    #[test]
    fn split_rects_vertical_divides_height() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = split_rects(area, 3, SplitOrientation::Vertical);
        assert_eq!(rects.len(), 3);
        // 24 / 3 = 8 each.
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[0].height, 8);
        assert_eq!(rects[1].y, 8);
        assert_eq!(rects[1].height, 8);
        assert_eq!(rects[2].y, 16);
        assert_eq!(rects[2].height, 8);
    }

    #[test]
    fn split_rects_distributes_remainder_to_first() {
        // 25 / 4 = 6, remainder = 1 → first rect is 7 wide.
        let area = Rect::new(0, 0, 25, 10);
        let rects = split_rects(area, 4, SplitOrientation::Horizontal);
        let total: u16 = rects.iter().map(|r| r.width).sum();
        assert_eq!(total, 25);
        assert_eq!(rects[0].width, 7);
        assert_eq!(rects[1].width, 6);
        assert_eq!(rects[2].width, 6);
        assert_eq!(rects[3].width, 6);
    }
}
