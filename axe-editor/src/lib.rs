pub mod buffer;
pub mod buffer_manager;
pub mod cursor;
pub mod diagnostic;
pub mod diff;
pub mod highlight;
pub mod history;
pub mod languages;
pub mod selection;

pub use buffer::{EditorBuffer, LineEnding};
pub use buffer_manager::BufferManager;
pub use cursor::CursorState;
pub use diagnostic::{BufferDiagnostic, DiagnosticSeverity};
pub use diff::{DiffHunk, DiffHunkKind};
pub use highlight::{HighlightKind, HighlightSpan};
pub use selection::Selection;
