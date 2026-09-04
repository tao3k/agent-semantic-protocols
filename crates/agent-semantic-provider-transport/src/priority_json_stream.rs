//! Bounded control-priority JSON stream for provider transports.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

/// Terminal reason emitted exactly once by a priority JSON stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PriorityJsonStreamTerminal {
    SequenceOverflow,
    EncodeFailed(String),
}

/// One bounded stream that always polls control messages before data messages.
pub struct PriorityJsonStream<T> {
    control: ReceiverStream<Value>,
    data: ReceiverStream<Value>,
    sequence: u64,
    encoder: fn(&Value) -> Result<T, String>,
    terminal: watch::Sender<Option<PriorityJsonStreamTerminal>>,
    stopped: bool,
}

impl<T> PriorityJsonStream<T> {
    #[must_use]
    pub fn new(
        control: mpsc::Receiver<Value>,
        data: mpsc::Receiver<Value>,
        sequence: u64,
        encoder: fn(&Value) -> Result<T, String>,
        terminal: watch::Sender<Option<PriorityJsonStreamTerminal>>,
    ) -> Self {
        Self {
            control: ReceiverStream::new(control),
            data: ReceiverStream::new(data),
            sequence,
            encoder,
            terminal,
            stopped: false,
        }
    }

    fn encode_next(&mut self, mut request: Value) -> Option<T> {
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.stop(PriorityJsonStreamTerminal::SequenceOverflow);
            return None;
        };
        self.sequence = sequence;
        let Some(request_object) = request.as_object_mut() else {
            self.stop(PriorityJsonStreamTerminal::EncodeFailed(
                "priority JSON request must be an object".to_owned(),
            ));
            return None;
        };
        request_object.insert("sequence".to_owned(), Value::from(sequence));
        match (self.encoder)(&request) {
            Ok(encoded) => Some(encoded),
            Err(error) => {
                self.stop(PriorityJsonStreamTerminal::EncodeFailed(error));
                None
            }
        }
    }

    fn stop(&mut self, reason: PriorityJsonStreamTerminal) {
        if !self.stopped {
            self.stopped = true;
            self.terminal.send_replace(Some(reason));
        }
    }
}

impl<T> Stream for PriorityJsonStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.stopped {
            return Poll::Ready(None);
        }
        let control_closed = match Pin::new(&mut self.control).poll_next(cx) {
            Poll::Ready(Some(item)) => return Poll::Ready(self.encode_next(item)),
            Poll::Ready(None) => true,
            Poll::Pending => false,
        };
        match Pin::new(&mut self.data).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(self.encode_next(item)),
            Poll::Ready(None) if control_closed => Poll::Ready(None),
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/priority_json_stream.rs"]
mod tests;
