# Canopa Greeter

An unprivileged, software-rendered fullscreen xdg-shell login interface for greetd.
Uses `canopa-session-ui` with Canopa Lock. Greetd owns PAM, account policy, privilege
changes and starting/closing the authenticated desktop. Supports manual/configured
users, configured sessions, keyboard/pointer actions and interactive PAM prompts.
Installed sessions and AccountsService/NSS user discovery populate the selectors;
manual usernames and explicit configuration remain available when discovery fails.
Selected-user avatars come from AccountsService, per-user configuration or a system
avatar directory, with an initials fallback and bounded asynchronous loading.
`canopa-greeter session --compositor cage|weston` launches the chosen kiosk;
the plain command connects to an existing compositor. The reproducible comparison
is documented in [compositor measurements](../../docs/greeter-performance.md).
See [setup, configuration and validation](../../docs/greeter.md).
