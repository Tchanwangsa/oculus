//! The consumer's alarm. Work that waits on time (a draft write, a settle
//! retry) asks for a [`BusMsg::Wake`]; the app's bus sends one when the time
//! comes and nothing else arrived first.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::BusMsg;

/// When the consumer next wants waking, shared with the bus it reads.
pub(super) type Alarm = Arc<Mutex<Option<Instant>>>;

/// The app's bus as the consumer reads it. Any message clears the alarm: the
/// consumer sets it again after each one, while something still waits.
pub(super) struct Wakeful {
    pub rx: Receiver<BusMsg>,
    pub alarm: Alarm,
}

impl Iterator for Wakeful {
    type Item = BusMsg;

    fn next(&mut self) -> Option<BusMsg> {
        let Some(at) = self.alarm.lock().unwrap().take() else {
            return self.rx.recv().ok();
        };
        match self
            .rx
            .recv_timeout(at.saturating_duration_since(Instant::now()))
        {
            Ok(msg) => Some(msg),
            Err(RecvTimeoutError::Timeout) => Some(BusMsg::Wake),
            Err(RecvTimeoutError::Disconnected) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn an_alarm_wakes_an_idle_bus_once_and_a_message_comes_first() {
        let (tx, rx) = std::sync::mpsc::channel();
        let alarm = Alarm::default();
        let mut bus = Wakeful {
            rx,
            alarm: alarm.clone(),
        };
        *alarm.lock().unwrap() = Some(Instant::now() + Duration::from_millis(5));
        assert!(matches!(bus.next(), Some(BusMsg::Wake)));
        assert!(alarm.lock().unwrap().is_none(), "one wake per alarm");

        *alarm.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        let turn = "1".to_string();
        tx.send(BusMsg::Bound {
            thread_id: 1,
            generation: 1,
            turn,
        })
        .unwrap();
        assert!(
            matches!(bus.next(), Some(BusMsg::Bound { .. })),
            "a message does not wait for the alarm"
        );
        drop(tx);
        assert!(bus.next().is_none(), "the bus closed");
    }
}
