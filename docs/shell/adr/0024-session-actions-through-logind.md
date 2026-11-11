# Session actions through logind

Date: 2026-11-11
Status: accepted

Suspend, hibernate, reboot and power off are a typed `session "ACTION"` panel
action (`suspend`, `hibernate`, `reboot`, `power-off`), usable in module actions
and menu items. The Shell calls the logind Manager methods on the system bus
with `interactive=true` and the D-Bus interactive-authorization flag, so polkit
can prompt when other users are logged in or an inhibitor needs overriding. It
does not run `systemctl` through the bounded command path: that path killed the
command after three seconds, often during the polkit prompt, and discarded its
result. The client follows `canopa-session`'s logind patterns (an uncached
Manager proxy, bounded calls) without sharing code yet; a later shared crate will
own both.

Visibility follows logind's `Can*` answers. They are read when the Shell starts
(only if the configuration uses a session action), whenever Quick settings opens,
and after a failed request. `no` and `na` hide the action; `yes`, `challenge` and
an unanswered question show it, so an unreachable logind still reports a failure
when used. Hibernate is shown only for a definite `yes`, because it is commonly
unconfigured and a failed resume loses the session. A button is hidden when its
click is a refused session action, or opens a menu whose session items are all
refused; menu items for refused actions are left out of that menu.

Reboot and Power off keep their confirmation menus, with Cancel first. Opening a
confirmation lists `ListInhibitors` entries in `block` mode for the action's lock
type (`shutdown` or `sleep`) under its prompt, for example "Blocked by Firefox:
Downloading files". Suspend and hibernate keep the session, so they run without a
confirmation. One request runs at a time.

A failed request is reported through the Shell's notice service (Notify, see
[ADR 0021](0021-live-configuration-reload.md)):
blocking applications when logind holds block inhibitors, "not authorized" for a
polkit refusal or cancelled prompt, logind's own message otherwise, and an
unavailable login manager. A successful request clears that notice. Success means
logind accepted the request, not that the system has suspended or shut down.

Log out still asks Niri to quit through its CLI; moving it to `niri-ipc` belongs
to the compositor integration work.
