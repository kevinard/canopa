//! Versioned provider contracts shared by Canopa applications.
//! Wire models have no toolkit or application dependency. Tokio I/O is opt-in.

mod error;
pub mod v1;
pub mod v4;

pub use error::ProtocolError;
// Preserve the original v4 Rust imports as well as its unchanged wire format.
pub use v4::*;
