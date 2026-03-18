pub mod buffer;
pub mod buffer_manager;
pub mod cursor;
pub mod diagnostic;
pub mod highlight;
pub mod history;
pub mod languages;
pub mod selection;

pub use buffer::EditorBuffer;
pub use buffer_manager::BufferManager;
pub use cursor::CursorState;
pub use diagnostic::{BufferDiagnostic, DiagnosticSeverity};
pub use highlight::{HighlightKind, HighlightSpan};
pub use selection::Selection;
