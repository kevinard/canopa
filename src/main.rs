mod app;
mod config;
mod dbus;
mod modules;
mod niri;
mod surface;

use app::App;
use iced_layershell::build_pattern::application;
use iced_layershell::reexport::Anchor;
use iced_layershell::settings::{LayerShellSettings, Settings};

fn main() -> iced_layershell::Result {
    application(App::new, App::namespace, App::update, App::view)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                // Anchor to left edge, stretch full screen height
                anchor: Anchor::Left | Anchor::Top | Anchor::Bottom,
                // Reserve 48px on the left so tiled windows don't overlap the bar
                exclusive_zone: 48,
                // 48px wide; 0 on height = stretch between top and bottom anchors
                size: Some((48, 0)),
                ..Default::default()
            },
            ..Default::default()
        })
        .style(App::style)
        .run()
}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_entry_point_compiles() {
        // Smoke test: if this test runs, the binary entry point compiled
        assert!(true);
    }
}
