/// Catppuccin Mocha colour palette constants.
///
/// All values are `const` so they can be used in match arms, array initialisers, and
/// anywhere else a constant expression is required.
use iced::Color;

/// Blue accent — used for active/normal-state module icons and focused workspace pills.
pub const ACCENT: Color = Color::from_rgb8(0x89, 0xb4, 0xfa);
/// Primary text colour.
pub const TEXT: Color = Color::from_rgb8(0xcd, 0xd6, 0xf4);
/// Secondary/subdued text — labels in expanded module rows.
pub const SUBTEXT: Color = Color::from_rgb8(0xa6, 0xad, 0xc8);
/// Darkest background — bar background colour and badge text on critical pills.
pub const BASE: Color = Color::from_rgb8(0x1e, 0x1e, 0x2e);
/// First surface layer — module hover/focus frame background.
pub const SURFACE0: Color = Color::from_rgb8(0x31, 0x32, 0x44);
/// Second surface layer — bar border strip.
pub const SURFACE1: Color = Color::from_rgb8(0x45, 0x47, 0x5a);
/// Dim overlay — muted/zero-volume icon.
pub const OVERLAY0: Color = Color::from_rgb8(0x6c, 0x70, 0x86);
/// Error / critical state — low battery badge, battery-critical icon, network-offline icon.
pub const CRITICAL: Color = Color::from_rgb8(0xf3, 0x8b, 0xa8);
/// Warning state — low-battery icon, limited-network icon.
pub const WARNING: Color = Color::from_rgb8(0xf9, 0xe2, 0xaf);
/// Success / charging state — battery-charging icon.
pub const SUCCESS: Color = Color::from_rgb8(0xa6, 0xe3, 0xa1);
