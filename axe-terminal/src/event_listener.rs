// IMPACT ANALYSIS — event_listener module
// Parents: TerminalTab creates a PtyEventListener when building a Term<PtyEventListener>.
// Children: alacritty_terminal's Term calls send_event() on state changes (DSR replies,
//           title changes, bell, etc.). PtyEvent is consumed by TerminalTab::drain_pty_events().
// Siblings: tab.rs drains the receiver end after each process_output() call.

use std::sync::mpsc;

use alacritty_terminal::event::{Event, EventListener};

/// Events forwarded from the terminal emulator to the PTY owner.
#[derive(Debug, PartialEq)]
pub enum PtyEvent {
    /// Data to write back to the PTY master (e.g., DSR cursor position response).
    Write(String),
    /// Terminal title change (OSC 0/2).
    Title(String),
    /// Terminal bell (BEL character).
    Bell,
}

/// Event listener that forwards relevant alacritty_terminal events over an mpsc channel.
///
/// This replaces the no-op `AltScreenListener`. The receiver end is owned by `TerminalTab`,
/// which drains it after each `process_output()` call.
pub struct PtyEventListener {
    tx: mpsc::Sender<PtyEvent>,
}

impl PtyEventListener {
    /// Creates a new listener that sends events to the given channel.
    pub fn new(tx: mpsc::Sender<PtyEvent>) -> Self {
        Self { tx }
    }
}

impl EventListener for PtyEventListener {
    fn send_event(&self, event: Event) {
        let pty_event = match event {
            Event::PtyWrite(s) => PtyEvent::Write(s),
            Event::Title(s) => PtyEvent::Title(s),
            Event::Bell => PtyEvent::Bell,
            _ => return,
        };

        // Receiver may be dropped if the tab is being torn down — that's fine.
        let _ = self.tx.send(pty_event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_event_listener_forwards_pty_write() {
        let (tx, rx) = mpsc::channel();
        let listener = PtyEventListener::new(tx);

        listener.send_event(Event::PtyWrite("\x1b[1;1R".to_string()));

        let event = rx.try_recv().expect("should receive PtyEvent::Write");
        assert_eq!(event, PtyEvent::Write("\x1b[1;1R".to_string()));
    }

    #[test]
    fn pty_event_listener_forwards_title() {
        let (tx, rx) = mpsc::channel();
        let listener = PtyEventListener::new(tx);

        listener.send_event(Event::Title("my-shell".to_string()));

        let event = rx.try_recv().expect("should receive PtyEvent::Title");
        assert_eq!(event, PtyEvent::Title("my-shell".to_string()));
    }

    #[test]
    fn pty_event_listener_forwards_bell() {
        let (tx, rx) = mpsc::channel();
        let listener = PtyEventListener::new(tx);

        listener.send_event(Event::Bell);

        let event = rx.try_recv().expect("should receive PtyEvent::Bell");
        assert_eq!(event, PtyEvent::Bell);
    }

    #[test]
    fn pty_event_listener_ignores_wakeup() {
        let (tx, rx) = mpsc::channel();
        let listener = PtyEventListener::new(tx);

        listener.send_event(Event::Wakeup);

        assert!(
            rx.try_recv().is_err(),
            "Wakeup should not produce a PtyEvent"
        );
    }

    #[test]
    fn pty_event_listener_does_not_panic_on_dropped_receiver() {
        let (tx, rx) = mpsc::channel();
        let listener = PtyEventListener::new(tx);
        drop(rx);

        // Should not panic.
        listener.send_event(Event::PtyWrite("data".to_string()));
        listener.send_event(Event::Bell);
    }
}
