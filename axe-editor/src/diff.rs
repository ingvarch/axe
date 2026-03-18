/// Represents the type of change for a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHunkKind {
    /// Lines that were added (not present in HEAD).
    Added,
    /// Lines that were modified (different content from HEAD).
    Modified,
    /// Lines that were deleted (present in HEAD but removed).
    Deleted,
}

/// Represents a contiguous range of changed lines in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// 0-based start line in the current buffer.
    pub start_line: usize,
    /// Number of lines affected in the current buffer.
    /// For `Deleted` hunks this is 0 (the deletion point is at `start_line`).
    pub line_count: usize,
    /// Type of change.
    pub kind: DiffHunkKind,
}

/// Returns the `DiffHunkKind` for a given line, or `None` if unchanged.
pub fn diff_kind_for_line(hunks: &[DiffHunk], line: usize) -> Option<DiffHunkKind> {
    for hunk in hunks {
        match hunk.kind {
            DiffHunkKind::Deleted => {
                // Deleted hunks have line_count 0; they mark the deletion point.
                if hunk.line_count == 0 && line == hunk.start_line {
                    return Some(DiffHunkKind::Deleted);
                }
            }
            _ => {
                if line >= hunk.start_line && line < hunk.start_line + hunk.line_count {
                    return Some(hunk.kind);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_hunks_returns_none() {
        assert_eq!(diff_kind_for_line(&[], 0), None);
        assert_eq!(diff_kind_for_line(&[], 42), None);
    }

    #[test]
    fn added_line_returns_added() {
        let hunks = vec![DiffHunk {
            start_line: 5,
            line_count: 3,
            kind: DiffHunkKind::Added,
        }];
        assert_eq!(diff_kind_for_line(&hunks, 5), Some(DiffHunkKind::Added));
        assert_eq!(diff_kind_for_line(&hunks, 6), Some(DiffHunkKind::Added));
        assert_eq!(diff_kind_for_line(&hunks, 7), Some(DiffHunkKind::Added));
    }

    #[test]
    fn modified_line_returns_modified() {
        let hunks = vec![DiffHunk {
            start_line: 2,
            line_count: 1,
            kind: DiffHunkKind::Modified,
        }];
        assert_eq!(diff_kind_for_line(&hunks, 2), Some(DiffHunkKind::Modified));
    }

    #[test]
    fn deleted_at_line_returns_deleted() {
        let hunks = vec![DiffHunk {
            start_line: 3,
            line_count: 0,
            kind: DiffHunkKind::Deleted,
        }];
        assert_eq!(diff_kind_for_line(&hunks, 3), Some(DiffHunkKind::Deleted));
    }

    #[test]
    fn line_outside_hunks_returns_none() {
        let hunks = vec![
            DiffHunk {
                start_line: 2,
                line_count: 2,
                kind: DiffHunkKind::Added,
            },
            DiffHunk {
                start_line: 10,
                line_count: 1,
                kind: DiffHunkKind::Modified,
            },
        ];
        assert_eq!(diff_kind_for_line(&hunks, 0), None);
        assert_eq!(diff_kind_for_line(&hunks, 1), None);
        assert_eq!(diff_kind_for_line(&hunks, 4), None);
        assert_eq!(diff_kind_for_line(&hunks, 9), None);
        assert_eq!(diff_kind_for_line(&hunks, 11), None);
    }

    #[test]
    fn multiple_hunks_returns_correct_kind() {
        let hunks = vec![
            DiffHunk {
                start_line: 0,
                line_count: 2,
                kind: DiffHunkKind::Added,
            },
            DiffHunk {
                start_line: 5,
                line_count: 0,
                kind: DiffHunkKind::Deleted,
            },
            DiffHunk {
                start_line: 8,
                line_count: 3,
                kind: DiffHunkKind::Modified,
            },
        ];
        assert_eq!(diff_kind_for_line(&hunks, 0), Some(DiffHunkKind::Added));
        assert_eq!(diff_kind_for_line(&hunks, 1), Some(DiffHunkKind::Added));
        assert_eq!(diff_kind_for_line(&hunks, 5), Some(DiffHunkKind::Deleted));
        assert_eq!(diff_kind_for_line(&hunks, 8), Some(DiffHunkKind::Modified));
        assert_eq!(diff_kind_for_line(&hunks, 10), Some(DiffHunkKind::Modified));
        assert_eq!(diff_kind_for_line(&hunks, 3), None);
    }
}
