mod app;
mod config;
mod dbus;
mod modules;
mod niri;
mod surface;

// Verify iced_layershell linkage at compile time (AC4, AR3)
use iced_layershell as _;

fn main() -> anyhow::Result<()> {
    // Story 1.2 will replace this body with iced_layershell app setup
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_entry_point_compiles() {
        // Smoke test: if this test runs, the binary entry point compiled
        assert!(true);
    }
}
