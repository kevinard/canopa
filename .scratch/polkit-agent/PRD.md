# Canopa polkit authentication agent

Status: implemented and validated within the recorded scope.

Added canopa-polkit, an unprivileged Wayland agent for an owned graphical logind
session, including user-manager discovery and explicit disambiguation. It uses
native PolkitAgentListener/PolkitAgentSession, the distribution's privileged
backend and polkit-1 PAM policy. Only the authority-selected Unix-user identities
are selectable. No Root action is executed or privilege receipt fabricated by
Canopa; a normally completed conversation without a helper receipt remains denied.

A small native C adapter owns registration and session callbacks on its own GLib
main context. A unique-owner filter protects the whole agent object path, including
calls without a D-Bus interface header. Request/body/identity/prompt/input limits,
one admitted request, generation/prompt checks, three-attempt retry limits,
cancellation, timeout and bounded failed-backend shutdown protect the lifecycle.
Canopa input/native dispatch buffers are erased; debug data and unrelated endpoints
are removed before native startup, with dump protection for normal agent operation.
Native library/PAM allocations and helper lifecycle remain native-backend concerns.

The ordinary Wayland dialog exists only during a request. It reuses shared Canopa
appearance, layout, erased input and prompt presentation, including an explicit
logical-scale entry point for dialogs while preserving fullscreen consumers.
Context/details paging, identity selection, secret/visible prompts, hardware-only
factors, pointer/keyboard/repeat/caps handling and buffer retirement are implemented.
No custom scripts, account roster polling, wallpaper process or idle render loop
is started. The shared configuration owns a polkit namespace (font/auth-timeout).
The forwarding CLI, release/updater, dependency notices, session target/unit,
readiness, CI prerequisites, maintained desktop manifest and documentation include
the agent. The service is packaged, not enabled on this host.

Validation:

- **21 focused checks passed**: shared config intake/theme (12), shared presentation
  (4), agent state (3), configuration/preview isolation (1) and native protocol (1).
  The native test's explicitly ignored child is run only by its private parent.
  Final agent checks are in agent-final.log; the broader focused run is in
  focused-final.log. The native parent now runs **nine scenarios**: password/OTP,
  retry, complete denial without any receipt, allowed identity switch/stale answer,
  user and authority cancellation, device-only authentication, oversized metadata,
  and overlapping-request refusal. Each also checks ordinary and omitted-interface
  foreign callers and the per-process runtime guard. No credential text is logged.
- **22 maintained native desktop checks passed** through run-desktop-acceptance.py
  (maintained-final/). Actual Niri windows/input verify preview, owned/foreign/
  ambiguous discovery, idle/no-window state, duplicate refusal, OTP, identity
  choice, retry, full details, devices, user/authority cancellation, timeout,
  request retirement, readiness, logout, blocked cancellation and fresh starts
  after backend/authority loss. Synthetic screenshots were visually inspected.
- **Six real-helper/PAM checks passed**, helper-final/: the installed native
  PolkitAgentSession and helper run in private user/PID/mount/network namespaces
  with synthetic passwd/nsswitch and polkit-1 PAM files. They exercise actual
  wrong-password retry, secret/visible OTP, a device-only factor and the helper's
  receipt to the private authority. No session API mock or host PAM is used in this
  test. The canonical system-bus socket is private inside the mount namespace,
  accommodating the privileged helper's environment sanitization.
- **18 greeter regressions passed**, greeter-regression.log. Strict workspace
  Clippy passed (clippy-final.log). Updater staging/service behavior passed 20 tests;
  unit ordering, check-script/desktop-runner tests, seven-owner configuration,
  settings-portal activation/read/reload and vendor regressions passed.
  Rust/Python/shell syntax and diff whitespace passed. Documentation staging has
  42 files / 59 offline links. The 605-entry dependency inventory is current;
  native polkit/GLib remain distribution system libraries, without a vendor fork.

The required full workspace gate ran once after the shared/application integration
(workspace.log). Preflights, strict lint and inventory checks passed. Cargo's full
suite stopped at the previously recorded Files recovery_catalog test
`pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`, with
"Recovery catalog did not settle". This work changes no Files operation behavior
and does not establish a successful full-workspace/release gate. Remaining portal,
configuration, vendor and executable-build checks were completed separately.

Development failures were corrected: initial SCTK constructor/error conversions,
a test authority's incorrectly object-path-typed registration argument (the native
protocol uses a string), the test keyboard's missing Right key, and a callback
Clippy suggestion. The real-helper fixture initially tried a read-only /tmp bus
socket and then an environment-selected bus ignored by the privileged helper;
it now uses the namespace's private /run/dbus/system_bus_socket and passes.
The final native filter additionally covers omitted interface headers, and the
terminal-failure case verifies that successful RPC completion alone grants nothing.

The native/private tests do not validate the deployment's real action rules,
privilege switching, hardware enrollment or PAM configuration. Shared presentation
currently has no AT-SPI tree or complex-script shaping. Existing compositor
benchmarks were not rerun or presented as polkit measurements.

No host agent was enabled/replaced, no host polkit rule/PAM/account/avatar file
was changed, and no privileged host action was performed. Authentication/image
fixture data is synthetic. Activation and deployment are documented in docs/polkit.md.
