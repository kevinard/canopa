/// The bundled default configuration, embedded at compile time.
/// Used when no user config file is present at the XDG path.
pub(super) const DEFAULT_CONFIG: &str = include_str!("../../config/default.kdl");
