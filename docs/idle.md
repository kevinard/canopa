# Idle policy with Stasis

Canopa uses the external [Stasis daemon](https://github.com/saltnpepper97/stasis)
for idle policy. `canopa-idle` supervises it and supplies Canopa/Niri commands.
The lock listener owns session-lock readiness and logind sleep coordination, so
restarting Stasis does not remove the listener's delay inhibitor. Stasis is optional
and is not bundled into Canopa binaries or release archives.

Install Stasis 1.7, `canopa-idle`, `canopa-lock`, Niri and the distribution's
`/etc/pam.d/canopa-lock` policy, then create a configuration:

```sh
canopa idle init
# Edit ~/.config/canopa/stasis.rune, then:
systemctl --user daemon-reload
systemctl --user start canopa-lock.service canopa-idle.service
canopa idle info --json
canopa idle pause
canopa idle resume
canopa idle profile work
canopa idle reload
```

Install `packaging/systemd/canopa-idle.service` alongside the other Canopa user
units. The updater and release archive include it. The session target wants this
unit; it stays inactive until the Rune file and PAM service exist. `init` creates
the Rune file with mode 0600 and refuses to overwrite it. For manual operation,
run `canopa-lock daemon` followed by `canopa-idle run`; `--config FILE`,
`--stasis /absolute/path/to/stasis` and `--session ID` support explicit selection.
Use one Stasis instance per user runtime directory. Do not also start its upstream
service. Stop `canopa-idle.service` to stop its owned backend. Losing the selected
lock coordinator stops that backend; an already active locker remains locked.

The defaults lock after five minutes, turn monitors off one minute later and
request suspend ten minutes after that. Battery defaults are two minutes,
30 seconds and five minutes respectively. Timeouts are sequential. Games,
gamepads, media and application polling detectors are disabled. Stasis still has
its own housekeeping timers. Enable detectors in Rune when their inhibition
behavior is wanted; Canopa does not duplicate their policy in KDL. Run
`canopa idle watch` to consume streaming state, and `canopa idle toggle` to toggle
the explicit manual hold. Automatic application/media inhibition remains distinct
from that manual hold.

The optional shell module uses the same stream and manual hold:

```kdl
shell {
    modules {
        module "awake" {
            type "idle-inhibitor"
            backend "stasis"
        }
    }
    bar "top" { modules { start "awake"; }; }
}
```

`backend "wayland"` remains the default for existing modules. The Stasis backend
starts a watcher only while a configured module consumes it. Closing the shell
does not release Stasis's manual hold. Automatic holds appear as “Inhibited”; the
tooltip includes the current profile, state and manual hold. Failed connections
show unavailable and reconnect with bounded backoff.

`canopa idle suspend` waits for an actual compositor lock acknowledgement before
calling logind's `Suspend(true)`. It refuses if locking fails or times out. Other
sleep sources use the lock listener's `Inhibit("sleep", …, "delay")` descriptor:
on `PrepareForSleep(true)` it locks and releases the descriptor only after the
compositor acknowledges. Resume acquires a fresh descriptor. A failed locker
never produces a readiness acknowledgement. Logind controls the maximum delay
and any forced-sleep policy; a delay inhibitor cannot veto sleep indefinitely.
The compositor's session lock remains the security boundary, and PAM alone
authorizes unlock. Starting a process or setting `LockedHint` is not readiness.

The maintained native scenario `session-stasis` uses a real external Stasis,
private Weston/Niri, a private logind service with real FD inhibitors and isolated
PAM policies. Supply `--stasis FILE` to `scripts/run-desktop-acceptance.py`.
The tested upstream revision is `d84e75e22cc00329712fb1caa83445d2f12ce921`
(1.7.0); acceptance records the executable version and SHA-256. Physical
suspend/resume and distribution policy acceptance require a separate test seat.

Canopa defaults Stasis and its IPC clients to one Tokio worker when
`TOKIO_WORKER_THREADS` is unset; a supplied value is preserved. The external
backend remains an unmodified upstream executable. A private debug comparison
of the supervisor/backend/readiness-watch stack found 8 threads with one worker
versus 14 with the upstream four-worker default, with similar memory usage.
Polling detectors were the larger CPU cost. See
[the scoped measurements](performance/data/2026-11-01-session/stasis-summary.json)
and [raw samples](performance/data/2026-11-01-session/stasis-trace.json.gz).
These are unoptimized measurements, not a production CPU guarantee; scheduler
context switches are a sleep proxy, not hardware wakeups.
Direct scheduler-wakeup tracing was unavailable on the measurement host because
the tracepoint was not readable; no kernel tracing permissions were changed.
