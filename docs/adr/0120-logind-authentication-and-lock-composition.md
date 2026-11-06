# 0120: Logind locking, interactive PAM and lockscreen composition

Status: implemented; validation is recorded in `.scratch/lockscreen-integration/PRD.md`.

The session tools introduced in ADR 0119 need standard lock requests, hardware and
multi-factor PAM conversations, configurable placement and passive custom widgets.

`canopa-lock daemon` subscribes to `org.freedesktop.login1.Session.Lock` on the real
owned session object. A Type=notify user service supplies this companion. Discovery
accepts an explicit session or environment, resolves the caller's PID, or requires
one unambiguous owned graphical session for user-service contexts. A private flock
prevents duplicate listeners. Requests spawn the ordinary locker; stopping the
listener preserves an active lock. Unlock signals never authorize unlocking.

The locker reports LockedHint only after ext-session-lock acknowledgement and clears
it after a successful compositor roundtrip for authenticated unlock. It retries
logind connections and restores confirmed state on reconnection. The listener also
observes child readiness and exit. Crashes leave the bus hint set but retire the
listener's private copy, so a direct recovery locker can report subsequent state.
Live owners reconnect and restore state; an abandoned crashed lock has no live
reporter if logind also loses its stored hint. Logind hints remain advisory; the
compositor is the security boundary. Idle/suspend inhibitors
remain outside this companion's responsibility.

Each authentication attempt runs in a separate child with a private inherited
socket, bounded framed events and fixed-capacity erased response buffers. PAM's
hidden and visible prompts each get a new response; informational/error messages
are rendered. PAM success without a password supports device-only policies.
Authentication and account validation both gate unlock. Escape, timeout and worker
death cancel the whole transaction; process-group termination, parent-death signals
and generation checks prevent blocked workers or stale results from unlocking.
The UI does not implement parallel competing PAM policies, device enrollment or
expired-password changes. Module installation and PAM control flow remain host policy.

Layout configuration assigns ordered content to nine viewport anchors with row or
column flow, bounded margins/gaps/offsets and output scaling. Authentication must
occur exactly once, remains inside the viewport and wins overlap conflicts. Existing
configurations retain central authentication and bottom widgets. Preview uses the
same planner and renderer without acquiring a session lock or starting PAM.

`canopa-widgets::script` adds a reusable bounded passive command reader. Each enabled
custom widget owns argv, interval, timeout and presentation settings. Commands start
only after locking and receive no input or authentication descriptors. UTF-8 output
and execution time are bounded, unchanged values do not redraw, and command groups
are cancelled on timeout/normal exit. The command leader dies with the consumer;
intentionally detached descendants remain the program's responsibility. Commands are
trusted configuration under the current user's privileges. Shell action dispatch,
Module/Panel state and interactive providers are not imported into the locker.

Private-compositor acceptance uses real loginctl against a test-owned logind service
and mount-isolated PAM modules for OTP, touch and fingerprint-style completion.
It validates protocol/conversation behavior; physical-device and distribution-policy
acceptance still belongs to the target machine.
