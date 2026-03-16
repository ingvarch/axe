// IMPACT ANALYSIS — event_listener module
// Parents: TerminalTab creates an AltScreenListener when building a Term<AltScreenListener>.
// Children: alacritty_terminal's Term calls send_event() on state changes.
// Siblings: None — this is a minimal bridge. Future tasks may extend it to forward
//           events (e.g., PtyWrite, Title) to the main event loop.

use alacritty_terminal::event::{Event, EventListener};

/// Minimal event listener for alacritty_terminal's `Term`.
///
/// Currently ignores all events. Future tasks will route `PtyWrite` events
/// back to the PTY master and `Title` events to the tab title.
pub struct AltScreenListener;

impl EventListener for AltScreenListener {
    fn send_event(&self, _event: Event) {
        // Intentionally empty for Task 6.1.
        // Task 6.2 will forward PtyWrite events to the PTY master.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_listener_can_be_instantiated() {
        let _listener = AltScreenListener;
    }

    #[test]
    fn event_listener_send_event_does_not_panic() {
        let listener = AltScreenListener;
        listener.send_event(Event::Wakeup);
        listener.send_event(Event::Bell);
    }
}
