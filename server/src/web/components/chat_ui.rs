//! Shared agent-chat UI, re-exported from the reusable `plan-ai-chat-ui`
//! crate. Both the healer pages (`healer_page`, `healer_sse`) and the fleet
//! chatbot (`chat_page`, `chat_sse`) consume this module; the legacy `PinInfo`
//! / `RunningToolInfo` aliases keep the local component code stable.

pub use plan_ai_chat_ui::render::{
    ChatMsg, ModelEntry, reason_display, render_message, simple_md_to_html, state_badge,
};
pub use plan_ai_chat_ui::wire::{
    ChatPin as PinInfo, ChatRunningTool as RunningToolInfo, ChatStreamEvent,
};

#[cfg(feature = "server")]
pub use plan_ai_chat_ui::convert::{
    approval_request_event, chat_event_to_stream, extract_pins_from_messages,
    running_tools_to_wire,
};
