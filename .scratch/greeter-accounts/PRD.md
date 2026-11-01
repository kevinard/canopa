# AccountsService user discovery

Status: implemented; private-bus, chooser-state and both native suites passed

Read AccountsService's cached non-system users asynchronously at greeter startup.
Retain configured/default usernames and manual entry without making the service a
login dependency. Bound calls and account metadata; use display names only for
presentation while greetd receives canonical usernames. Do not change an active
conversation, an edited username or a configured selection when results arrive.
No polling, account mutations, credentials, home/avatar reads or NSS enumeration.
Add a read-only list-users diagnostic, a discovery privacy switch, private-bus
acceptance, and native chooser/handoff/fallback tests. PAM remains authentication
authority. No host account or service configuration is changed.

## Result

The optional AccountsService reader queries ListCachedUsers and individually reads
Uid, UserName, SystemAccount and RealName. User-property requests are pinned to the
manager reply's unique owner and do not use GetAll. UID-zero/system accounts and
invalid usernames are excluded; non-local or low-UID human accounts remain
eligible. Display names are bounded presentation data; canonical usernames alone
enter the greetd transaction. No lock/password/expiry policy is inferred from
metadata; PAM remains authoritative.

The frontend starts a cancellable, one-shot asynchronous worker. Discovery has a
two-second global deadline, 128-path/chooser limits and eight concurrent user
reads. The connection/runtime retire after the read, with no polling. Configured
users retain order and default usernames retain prefilling priority. An untouched
empty chooser may prefill its first result but never begins authentication.
Typed/cleared usernames, active prompts, secrets, generations and selected
authentication accounts survive late metadata. Keyboard/pointer selection uses
the same merged list. Manual entry always remains available.

`discover-users #false` disables builtin queries and keeps configured users.
Schema checks and previews never contact AccountsService. `list-users` explicitly
inspects the merged roster without Wayland/greetd. The packaged example enables
discovery. Documentation describes optional installation, cached-list scope,
privacy and fallback. The direct zbus dependency was already transitively present;
Cargo.lock and the 604-dependency inventory are updated without adding a new
third-party package/version.

## Validation (2026-11-01)

- Focused greeter tests: **26 passed**, comprising 11 library tests, 3 independent
  private-bus AccountsService tests, 2 compositor tests, 2 configuration tests and
  8 desktop-session tests. See `focused.log`. Account coverage includes canonical
  usernames versus display names; active password/input preservation; root/system
  filtering without arbitrary UID/local-account restrictions; manual priority;
  duplicate/budget handling; invalid/missing display metadata; and absent, denied,
  oversized or delayed service fallback. Disabled/schema/preview paths make zero
  AccountsService calls. The peer rejects mutation and reads of unneeded metadata.
- **28 native checks passed under Cage and 28 under Weston** through the product
  launcher. `cage-final/`, `weston-final/`. The suite includes account-only user
  selection with display-name capture, exact canonical greetd handoff, late data
  during manual username input, manual authentication after denial/timeout, and
  all prior session-discovery/authentication/lockout/cancellation checks.
- An initial account screenshot caught the nested window resizing and a stale
  keypress frame. The fixture now opens windows fullscreen from creation and waits
  for account-name redraw before capturing. Both full matrices passed again; the
  final screenshot shows Bob Builder with canonical username bob at full size.
- Strict `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed;
  see `clippy.log`. Rust formatting, diff whitespace and Python compilation passed.
- Packaged greeter configuration validates. Documentation staging: **38 files,
  56 offline links**. The updated locked dependency inventory check passed.
- Read-only real system-bus probe through the unified CLI exited successfully with
  manual fallback because AccountsService was unavailable. No real account names,
  display names or UIDs were recorded. `validation.json` retains only the count and
  fallback status; all other account evidence is synthetic fixture data.

The existing full-workspace Files recovery and host AT-SPI failures remain recorded
in `.scratch/greeter/PRD.md`; that long test gate was not repeated for this greeter
feature. Strict workspace Clippy and the targeted/native checks above do not claim
that those failures are resolved. No shared Rust crate/fork or authentication
transport was changed. Installed daemon/PAM policy remains target-machine
acceptance; the private peer independently exercises the public D-Bus contract.

Account snapshots refresh at frontend startup or explicit inspection, rather than
tracking account/service changes while the same login window remains open. Manual
entry covers uncached/new/remote accounts and unavailable discovery. The historical
compositor benchmark is explicitly marked as predating session and user discovery;
no updated performance figures are claimed.

No host user accounts, services, display-manager configuration or PAM were modified.
