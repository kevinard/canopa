# Clipboard session integration

Status: private RemoteDesktop integration passes scoped acceptance, 2026-11-01.
Items 1–4 are implemented: Portal 172 library/48 integration tests, formatting,
Clippy and 62 native groups pass. Both appearances, retained-FD revocation and
existing input/media regressions pass on the final candidate. Items 5–7 retain
the broader failure/compatibility, InputCapture and release acceptance work.
See ADR0058 and README.md; normal registration remains disabled.

1. Add the exact impl Clipboard v1 interface on the existing sharing host. Resolve
   every session by object path plus authenticated original frontend unique name;
   reject missing, closed and ScreenCast-only sessions. Clipboard methods have no
   app_id argument, so use the stored session attribution. Reuse the session actor
   and its bounded command queue; use finite admission rather than accumulating
   method futures waiting for space.
2. Record RequestClipboard exactly once while configuring. Carry the request into
   RemoteDesktop Start and offer an independent, initially unchecked Iced choice.
   Return clipboard_enabled only for actual approval and a successfully opened
   transport. Add truthful ongoing sharing controls. Handle clipboard-only grants
   with no input/capture, without requiring unrelated recording capabilities.
   Pin the display to the compositor identity verified through the Niri IPC/input
   context; do not trust a fresh PID supplied by a portal caller.
3. Store the bridge in the approved control's lifetime. SelectionRead/Write use the
   correct outward Unix FD lists. Enforce owner/kind/phase before queuing work;
   validate MIME types and single-use serials. Session/request revocation must
   cancel pending bridge calls and stop the worker before acknowledging cleanup.
   Preserve success-after-EOF versus failure cancellation and the documented
   bounded in-flight policy.
4. Emit SelectionOwnerChanged and SelectionTransfer only to the original frontend,
   using the stored session handle. Filter stale generations and inactive grants.
   The pinned frontend clipboard.c:545-607 discards impl signals until its session
   is clipboard-enabled. Investigate the locked zbus 5.19.0
   object_server::ResponseDispatchNotifier to gate initial state delivery after
   the Start response is dispatched. Its notification does not establish peer
   receipt; verify ordering on the actual standard frontend instead of assuming
   a scheduler delay proves it.
5. Exercise public Clipboard through the canonical frontend, Iced consent and
   independent clipboard peers in both themes. Include wrong owners/kinds, duplicate
   requests, denial, source changes, FD direction/replay, large/empty binary data,
   slow/abandoned peers and app/frontend/backend/helper/compositor/UI loss. Keep
   all admitted FDs and streams finite after revocation. The current transport
   deliberately accepts pipe/Unix-stream targets; assess regular-file Wayland
   receive targets before declaring broad clipboard compatibility.
6. Connect InputCapture only to its real v2 session/capture implementation; enforce
   active-phase access and define revocation of already admitted transfers. Its
   compositor barrier/capture/EI-receiver boundary is still absent. Do not create a
   synthetic session kind to claim support. Normal Clipboard selection must not
   route another backend's InputCapture sessions to a foreign session registry.
7. Preserve existing capture/input regressions and required workspace gates after
   shared/cross-application changes. The latest broad workspace result is still red
   on ten Files tests; scoped Clipboard tests do not replace that result. Use the
   stable root incremental target and checksum-addressed compositor; keep concise
   receipts and avoid per-run executable copies. No live routing or installation
   is authorized by this implementation plan.
