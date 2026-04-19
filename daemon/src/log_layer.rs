use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use crate::log_buffer::LogBuffer;

/// A tracing layer that writes all log events to the in-memory LogBuffer.
pub struct BufferLayer {
    buf: LogBuffer,
}

impl BufferLayer {
    pub fn new(buf: LogBuffer) -> Self {
        Self { buf }
    }
}

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}

impl<S: Subscriber> Layer<S> for BufferLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = meta.level();
        let target = meta.target();

        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        // Skip noisy internal targets and service output (already in buffer via log_capture)
        if target.starts_with("rocket::") || target.starts_with("hyper::") || target == "service" {
            return;
        }

        let msg = strip_ansi_escapes::strip(&visitor.message);
        let msg = String::from_utf8(msg).unwrap_or(visitor.message);
        let line = format!("{level} {target}: {msg}");
        self.buf.push(line);
    }
}
