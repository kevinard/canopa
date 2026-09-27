# Systemd user service before D-Bus activation

Notify starts as an explicit systemd user service before adding D-Bus activation. This prioritizes predictable Wayland/session environment setup and early debugging over seamless activation on the first notification request. Version-one install artifacts include a systemd user unit, but not an autostart desktop file.
