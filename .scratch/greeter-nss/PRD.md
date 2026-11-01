# NSS user discovery

Status: implemented and validated.

The greeter supports `user-source "auto"|"accountsservice"|"nss"`. Default auto
mode prefers AccountsService and falls back only on unavailable/denied/failed/
timed-out reads. A successful empty cached-user list remains authoritative.
`discover-users #false` disables both sources; manual/configured users remain.
`list-users --source nss` provides an explicit diagnostic. Schema checks and
previews never start account discovery.

NSS uses libc enumeration in a fresh single-threaded role of the same executable,
before configuration, Wayland or authentication starts. It observes the installed
nameservice switch/providers without reading passwd files itself or querying shadow.
Only username, UID, shell eligibility and the first GECOS component are retained;
record buffers are erased. UID_MIN/MAX from bounded /etc/login.defs default to
1000..60000; nss-uid-min/max override them, and nss-include-nologin overrides the
normal nologin/false-shell filter. UID 0 is always omitted from discovery. Chooser
filters never restrict manually typed identities; greetd/PAM owns account policy.

The helper has a two-second deadline, fixed 64-KiB record/output budgets, at most
4096 examined records and 128 returned users. The frontend supervisor uses
nonblocking bounded reads and checks cancellation at most 50 ms apart. Process
exit is observed without reaping until cleanup owns the final group kill/reap,
avoiding PID reuse. Timeout, cancellation, malformed data or errors preserve manual
entry. Parent-death SIGKILL protects against a vanished frontend; dump protection
and endpoint removal match the other isolated session helpers. Discovery retires
its thread/connection once complete, without idle account polling.

Validation:

- **33 focused greeter tests passed**: 13 library, 4 private AccountsService,
  4 NSS, 2 compositor, 2 configuration and 8 session tests; `focused.log`.
  Synthetic NSS records cover canonical identity, limited GECOS, shell/UID bounds,
  duplicates, invalid encodings, enumeration/output caps, failed/blocked providers,
  process-group timeout cleanup, parent death and prompt cancellation/reaping.
  Source tests cover unavailable, denied and timed-out service fallback,
  authoritative empty/successful service responses, explicit NSS bypass and the
  master discovery switch.
- **Real libc namespace test passed**, `real-nss.log`: Bubblewrap provides private
  passwd/nsswitch/login.defs and PID/network namespaces. Actual getpwent_r honors
  UID defaults and nameservice routing without host NSS/account changes.
- **31 native checks passed on Cage and 31 on Weston** through the product launcher;
  each directory contains summary, private protocol events and screenshots. NSS
  chooser selection hands canonical bob to greetd; automatic fallback selects
  alice, and blocked NSS permits uncached manual login while helpers are reaped.
  NSS chooser screenshots show Bob NSS (bob) at full window size.
- Strict workspace Clippy passed (`clippy.log`), as did Rust formatting, diff
  whitespace, Python syntax, packaged greeter config validation, documentation
  staging (38 files / 56 offline links) and locked dependency inventory (604).
- A read-only real-host NSS probe passed; only count/shape booleans are retained
  (`host-probe.json`), never personal usernames, display names or UIDs. All other
  account records in the evidence are synthetic.

No shared Rust crate/fork or authentication transport changed in this increment.
The prior full-workspace Files recovery and host AT-SPI failures remain recorded
in `.scratch/greeter/PRD.md`; that long gate was not repeated here. Targeted/native
success and strict workspace Clippy do not establish their resolution. Native
fixtures do not validate physical seat/VT/DRM or a deployment's PAM/remote-directory
policy. Providers that disable enumeration require manual entry. The snapshot
refreshes at frontend startup or explicit inspection; historical compositor
benchmarks predate discovery and have not been replaced by new measurements.

No host users, NSS/PAM files, services or display-manager configuration changed.
