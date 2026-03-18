use super::EditorBuffer;

impl EditorBuffer {
    /// Scrolls the viewport by the given number of lines without moving the cursor.
    ///
    /// Positive delta scrolls down (later lines become visible),
    /// negative scrolls up (earlier lines become visible).
    /// Clamped to valid bounds.
    pub fn scroll_by(&mut self, delta: i32, viewport_height: usize) {
        let max_scroll = self.line_count().saturating_sub(viewport_height);
        if delta > 0 {
            self.scroll_row = (self.scroll_row + delta as usize).min(max_scroll);
        } else {
            self.scroll_row = self.scroll_row.saturating_sub((-delta) as usize);
        }
    }

    /// Sets the viewport width for horizontal scroll clamping.
    pub fn set_viewport_width(&mut self, w: usize) {
        self.viewport_width = w;
        self.clamp_scroll_col();
    }

    /// Scrolls the viewport horizontally by the given number of columns
    /// without moving the cursor.
    ///
    /// Positive delta scrolls right, negative scrolls left.
    /// Clamped so the view never scrolls past the longest line.
    pub fn scroll_horizontally_by(&mut self, delta: i32) {
        if delta > 0 {
            self.scroll_col += delta as usize;
        } else {
            self.scroll_col = self.scroll_col.saturating_sub((-delta) as usize);
        }
        self.clamp_scroll_col();
    }

    /// Clamps horizontal scroll so the view can't scroll past the longest line.
    fn clamp_scroll_col(&mut self) {
        let max_scroll = self.max_line_width().saturating_sub(self.viewport_width);
        self.scroll_col = self.scroll_col.min(max_scroll);
    }

    /// Adjusts scroll offsets to keep the cursor visible within the viewport.
    ///
    /// Maintains a margin of `SCROLL_MARGIN` lines from the viewport edges
    /// when possible.
    pub fn ensure_cursor_visible(&mut self, viewport_height: usize, viewport_width: usize) {
        const SCROLL_MARGIN: usize = 5;

        if viewport_height > 0 {
            // Vertical scrolling.
            if self.cursor.row < self.scroll_row + SCROLL_MARGIN {
                self.scroll_row = self.cursor.row.saturating_sub(SCROLL_MARGIN);
            }
            if viewport_height > SCROLL_MARGIN
                && self.cursor.row >= self.scroll_row + viewport_height - SCROLL_MARGIN
            {
                self.scroll_row = self.cursor.row + SCROLL_MARGIN + 1 - viewport_height;
            }
        }

        if viewport_width > 0 {
            // Horizontal scrolling.
            if self.cursor.col < self.scroll_col {
                self.scroll_col = self.cursor.col;
            }
            if self.cursor.col >= self.scroll_col + viewport_width {
                self.scroll_col = self.cursor.col + 1 - viewport_width;
            }
        }
    }
}
