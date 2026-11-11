//! Shared error and availability types.
//!
//! A service that is not installed, not running or not reachable is an ordinary
//! state ([`ServiceState::Unavailable`]) for observers and a typed
//! [`Error::Unavailable`] for commands. Pages show a placeholder for both.

/// What an observer currently knows about a service.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ServiceState<T> {
    /// The service has no owner on its bus, the bus is unreachable, or the last
    /// read failed. Observers keep watching and report recovery.
    #[default]
    Unavailable,
    Available(T),
}

impl<T> ServiceState<T> {
    pub fn available(&self) -> Option<&T> {
        match self {
            Self::Available(value) => Some(value),
            Self::Unavailable => None,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> ServiceState<U> {
        match self {
            Self::Available(value) => ServiceState::Available(map(value)),
            Self::Unavailable => ServiceState::Unavailable,
        }
    }
}

impl<T> From<Option<T>> for ServiceState<T> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Unavailable, Self::Available)
    }
}

/// Why a command or read failed. Service-specific failures with more detail
/// (Wi-Fi, logind session actions, audio) have their own types.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The bus or the service is not reachable.
    #[error("{}", fl!("services-error-unavailable"))]
    Unavailable,
    /// Polkit or the service refused, or the person cancelled authentication.
    #[error("{}", fl!("services-error-not-authorized"))]
    NotAuthorized,
    #[error("{}", fl!("services-error-timed-out"))]
    TimedOut,
    /// The device, profile or owner the request named is no longer current.
    /// Read the current state and ask again.
    #[error("{}", fl!("services-error-changed"))]
    Changed,
    /// The request is outside what the service accepts (for example a
    /// brightness of 0 %). The message is plain English for logs; Settings
    /// matches it to translate it, so it stays English.
    #[error("{0}")]
    Invalid(&'static str),
    /// The service refused for another reason; its bounded message is kept.
    #[error("{0}")]
    Refused(String),
}

impl Error {
    /// D-Bus errors that mean "nobody is there" rather than "no".
    pub(crate) fn is_absent_name(name: &str) -> bool {
        matches!(
            name,
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
                | "org.freedesktop.DBus.Error.UnknownObject"
                | "org.freedesktop.DBus.Error.Disconnected"
                | "org.freedesktop.DBus.Error.NoReply"
        )
    }

    pub(crate) fn is_denied_name(name: &str) -> bool {
        matches!(
            name,
            "org.freedesktop.DBus.Error.AccessDenied"
                | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
                | "org.freedesktop.PolicyKit1.Error.NotAuthorized"
                | "org.freedesktop.PolicyKit1.Error.Cancelled"
        )
    }

    fn method_error(name: &str, description: Option<&str>) -> Self {
        if Self::is_absent_name(name) {
            Self::Unavailable
        } else if Self::is_denied_name(name) {
            Self::NotAuthorized
        } else {
            let message = description
                .filter(|text| !text.trim().is_empty())
                .unwrap_or(name);
            Self::Refused(crate::bus::bounded_text(message, 200))
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, description, _) => {
                Self::method_error(name.as_str(), description.as_deref())
            }
            zbus::Error::FDO(error) => Self::from((**error).clone()),
            zbus::Error::InputOutput(_) | zbus::Error::Address(_) | zbus::Error::Handshake(_) => {
                Self::Unavailable
            }
            other => Self::Refused(crate::bus::bounded_text(&other.to_string(), 200)),
        }
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(error: zbus::fdo::Error) -> Self {
        use zbus::DBusError;
        let description = error.description().map(str::to_owned);
        Self::method_error(error.name().as_str(), description.as_deref())
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Self::TimedOut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_denied_and_other_method_errors_are_distinct() {
        let fdo = |error: zbus::fdo::Error| Error::from(zbus::Error::from(error));
        assert_eq!(
            fdo(zbus::fdo::Error::ServiceUnknown("gone".into())),
            Error::Unavailable
        );
        assert_eq!(
            fdo(zbus::fdo::Error::AccessDenied("no".into())),
            Error::NotAuthorized
        );
        assert_eq!(
            fdo(zbus::fdo::Error::Failed("busy\u{7}".into())),
            Error::Refused("busy ".into())
        );
        assert_eq!(
            ServiceState::from(Some(1)).map(|value| value + 1),
            ServiceState::Available(2)
        );
        assert!(!ServiceState::<u8>::default().is_available());
    }
}
