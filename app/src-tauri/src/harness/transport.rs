use std::process::{ChildStdout, Command};

use serde_json::Value;

use super::child::Process;
use super::{Provider, Sink};

/// The process boundary is replaceable in tests; protocol encoders and readers
/// stay in the provider bridges.
pub struct ChildProc {
    process: Option<Process>,
    #[cfg(test)]
    replay: Option<super::testkit::outbound::Wire>,
}

impl ChildProc {
    pub fn spawn(
        who: &'static str,
        cmd: &mut Command,
        stdin: bool,
    ) -> Result<(Self, ChildStdout), String> {
        let (process, stdout) = Process::spawn(who, cmd, stdin)?;
        Ok((
            Self {
                process: Some(process),
                #[cfg(test)]
                replay: None,
            },
            stdout,
        ))
    }

    #[cfg(test)]
    pub(in crate::harness) fn replay(wire: super::testkit::outbound::Wire) -> Self {
        Self {
            process: None,
            replay: Some(wire),
        }
    }

    pub fn is_alive(&self) -> bool {
        self.process.as_ref().is_none_or(Process::is_alive)
    }

    pub fn kill(&self) -> Option<i32> {
        self.process.as_ref().and_then(Process::kill)
    }

    pub fn reap(&self) -> Option<i32> {
        self.process.as_ref().and_then(Process::reap)
    }

    pub fn write_line(&self, value: &Value) -> Result<(), String> {
        #[cfg(test)]
        if let Some(wire) = &self.replay {
            return wire(value.clone());
        }
        self.process
            .as_ref()
            .expect("process transport")
            .write_line(value)
    }

    pub fn with_tail(&self, msg: String) -> String {
        match &self.process {
            Some(process) => process.with_tail(msg),
            None => msg,
        }
    }

    pub fn finish(
        &self,
        sink: &Sink,
        provider: Provider,
        mid_turn: impl FnOnce() -> Option<super::event::TurnId>,
    ) {
        match &self.process {
            Some(process) => process.finish(sink, provider, mid_turn),
            // A replayed transport closing: the same outcome, with no exit code.
            None => {
                if let Some(turn) = mid_turn() {
                    super::child::fail_turn(
                        sink,
                        provider,
                        "the replayed process exited mid-turn".into(),
                        Some(turn),
                    );
                }
                sink(super::HarnessEvent::Exited { code: None });
            }
        }
    }
}
