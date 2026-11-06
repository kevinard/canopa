//! Gate metadata on the backend's linked PipeWire library.
pub(super) fn cursor_modes() -> u32 {
    #[cfg(feature = "desktop")]
    {
        // Use the longstanding version getter, not pw_check_library_version,
        // whose symbol would raise our minimum ABI from 0.3.64 to 0.3.75.
        // SAFETY: PipeWire returns a static NUL-terminated version string.
        let pointer = unsafe { pipewire::sys::pw_get_library_version() };
        if pointer.is_null() {
            return 3;
        }
        // SAFETY: the non-null pointer above refers to the library's static string.
        unsafe { std::ffi::CStr::from_ptr(pointer) }
            .to_str()
            .ok()
            .map(modes_for_version)
            .unwrap_or(3)
    }
    #[cfg(not(feature = "desktop"))]
    {
        0
    }
}

#[cfg(any(feature = "desktop", test))]
fn modes_for_version(version: &str) -> u32 {
    let parse = || {
        let core = version.split(['-', '+']).next()?;
        let mut parts = core.split('.');
        let version = (
            parts.next()?.parse::<u32>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
        );
        parts.next().is_none().then_some(version)
    };
    // The pinned Niri capture service falls back to Embedded below 1.4.8.
    // Exclude Metadata rather than silently returning a different cursor mode.
    if parse().is_some_and(|version| version >= (1, 4, 8)) {
        7
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_requires_the_fixed_library_boundary_and_unknown_versions_are_conservative() {
        for version in [
            "0.3.64", "1.4.7", "unknown", "1.4", "1.4.8.1", "1.4.bad", "",
        ] {
            assert_eq!(modes_for_version(version), 3, "{version}");
        }
        for version in ["1.4.8", "1.4.8-1", "1.6.10+vendor.12", "2.0.0"] {
            assert_eq!(modes_for_version(version), 7, "{version}");
        }
    }
}
