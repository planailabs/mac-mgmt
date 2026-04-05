use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

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

        // Skip noisy internal targets
        if target.starts_with("rocket::") || target.starts_with("hyper::") {
            return;
        }

        let line = format!("{level} {target}: {}", visitor.message);
        self.buf.push(line);
    }
}
