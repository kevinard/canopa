//! Native AT-SPI trees for Iced windows through AccessKit.
//!
//! Views describe controls by wrapping them with [`semantic`]. One
//! [`Collect`] traversal of the laid-out widget tree then reads those nodes,
//! the text drawn inside them, the keyboard-focused widget and every node's
//! on-screen bounds. [`Bridge`] turns the result into one AccessKit tree per
//! window with node IDs kept stable by key, and sends each incoming request
//! back as the message mouse or keyboard input would send.
//!
//! ```ignore
//! semantic(
//!     Node::button("Save changes").key("settings-apply").on_click(save.clone()),
//!     button(text("Save changes")).on_press_maybe(save),
//! )
//! ```
//!
//! While no assistive technology listens the bridge collects nothing, and
//! `accesskit_unix` does not connect to the accessibility bus.
mod bridge;
mod collect;
mod harness;
mod node;
#[cfg(feature = "accessibility-testing")]
pub mod testing;
mod tree;
mod widget;

pub use accesskit::{Live, Role};
pub use bridge::{Bridge, Event, Message};
pub use collect::{Collect, Snapshot};
pub use harness::Harness;
pub use node::{Actions, Node, Range, SetValue, Value};
pub use widget::semantic;

#[cfg(test)]
mod tests;
