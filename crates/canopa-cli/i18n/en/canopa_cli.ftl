# Canopa Command-line interface. English is the source language.

## Help (`canopa --help`); the derive's doc comments in main.rs are the same text.

cli-about = Canopa desktop tools
cli-settings-about = Open Canopa Settings for devices, appearance, system and desktop preferences
cli-wallpaper-about = Wallpaper daemon and live image controls
# "preview" and "--help" are literal arguments.
cli-lock-about = Secure session lock; pass preview or --help for rendering tools
cli-greeter-about = Graphical greetd login interface and preview tools
cli-polkit-about = Graphical polkit authentication agent and preview tools
cli-pinentry-about = GnuPG PIN/passphrase dialog and preview tools
cli-idle-about = Stasis idle policy, streaming status and secure suspend integration
cli-config-about = Check or migrate the Canopa configuration
cli-config-check-about = Check all owning schemas. With no file, use the normal discovery order
cli-config-check-file = Shared Canopa file (never interpreted as a legacy file)
cli-config-check-show-source = Print local schema diagnostics, which may include configuration contents
cli-config-migrate-about = Wrap explicit legacy sources, check all schemas, then create a new file
cli-config-migrate-long-about = Wrap explicit legacy sources, check all schemas, then create a new file.
    Omitting a component uses its embedded defaults. Existing files are never replaced.

## Messages, printed after "canopa: " (errors start in lower case).

cli-check-failed = configuration check failed
cli-migrate-no-source = specify at least one legacy source
cli-migrate-too-large = combined configuration exceeds 1 MiB
cli-migrate-rejected = migration rejected by a component schema
# Printed after a successful migration. $path is the new file.
cli-migrate-created = Created { $path }. Install this candidate at the shared configuration path (or select it explicitly), then restart Canopa Shell and reload Notify.
cli-no-installation-directory = cannot find installation directory
cli-validator-cannot-run = component validator could not execute
cli-validator-invalid-report = component validator returned an invalid report
cli-validator-wrong-owner = component validator returned the wrong schema owner
cli-validator-inconsistent = component validator returned an inconsistent result
cli-install-all-executables = install all Canopa executables beside the canopa executable
cli-output-no-parent = output needs a parent directory
cli-output-exists = output already exists or cannot be created
# With --show-source. $component is a configuration section name (shell,
# notifications…), $path the file checked.
cli-show-source-copy = { $component }: { $path } (diagnostics refer to an immutable temporary copy)
