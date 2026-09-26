use serde::{Deserialize, Serialize};
use std::num::{NonZeroI64, ParseIntError, TryFromIntError};

/// A daemon identity, never a freedesktop wire ID. Positive values address retained
/// threads; negative values address runtime-only threads or immediately dropped
/// receipts. Zero is invalid. JSON retains the existing signed integer encoding.
///
/// Wire values must be resolved by the transport before entering Notification Intake:
/// ```compile_fail
/// use canopa_notify::NotificationInput;
/// let mut input = NotificationInput::default();
/// let wire_id = 42u32;
/// input.replaces_id = Some(i64::from(wire_id));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InternalNotificationId(NonZeroI64);

impl InternalNotificationId {
    pub const fn new(value: i64) -> Option<Self> {
        match NonZeroI64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Numeric representation for storage and control adapters, not wire conversion.
    pub const fn get(self) -> i64 {
        self.0.get()
    }

    pub const fn is_runtime_only(self) -> bool {
        self.get() < 0
    }
}

impl TryFrom<i64> for InternalNotificationId {
    type Error = TryFromIntError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        NonZeroI64::try_from(value).map(Self)
    }
}

impl std::str::FromStr for InternalNotificationId {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl std::fmt::Display for InternalNotificationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
