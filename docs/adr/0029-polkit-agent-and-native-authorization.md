# 0029: Polkit agent and native authorization

Status: implemented; validation is recorded in `.scratch/polkit-agent/PRD.md`.

Canopa provides a session authentication agent for privileged actions requested by
system services. Polkit remains the authority for action policy and permitted
identities, and its native session API performs credential verification and submits
the authorization receipt. Login, screen unlock and privileged-action authorization
retain separate policies and lifetimes.

`canopa-polkit` is a separate unprivileged application using the existing software
presentation, layout, appearance and erased input in `canopa-session-ui`. Its
native C adapter subclasses PolkitAgentListener and uses PolkitAgentSession on a
private GLib main context. It contains no alternative PAM stack or direct
AuthenticationAgentResponse implementation. A unique-owner bus filter supplements
the system's agent-interface policy. Registration uses an owned graphical logind
session, including discovery from user-manager services; ambiguous/foreign sessions
and another registered agent are refused.

The Wayland adapter maps an ordinary xdg-shell dialog only for an active request.
It renders the action/description, bounded authority-provided identity choices,
secret/visible prompts, notices and paged full details. Shared presentation gains
an explicit logical-scale rendering entry point for ordinary dialogs; existing
fullscreen callers retain their previous scale behavior. No custom widgets,
external commands, account roster discovery or wallpaper process is started by
the agent. The shared configuration gains a polkit owner; each existing owner
continues validating only its own namespace.

One request is admitted at a time. Native sessions are cancelled and disconnected
on identity changes or request end; generations and prompt serials reject stale
responses. Failed attempts have bounded retries/delay. Native timeouts and an
independent UI watchdog bound blocked cancellation/shutdown. Canopa-owned input
and native dispatch copies are erased, dumps are disabled, and debugging that can
expose conversation data is removed before library initialization. The native
backend retains responsibility for its own allocations and privileged helper.

The session unit reports readiness after registration, waits for compositor
readiness and stops with the graphical session. Authority loss ends the agent so
the service can register again with bounded restart frequency. Packaging/updating
includes the executable/unit and native development dependency. Implementation and
acceptance do not modify host rules, PAM, users, enrolled factors or enabled agents.

Private authority and conversation fixtures test the public registration/callback
contract independently of the UI. Native Wayland acceptance exercises actual
input/lifetime behavior. A namespace test additionally runs the real native
session/helper against synthetic account/PAM files and verifies its receipt to a
private authority. These scopes do not establish deployed policy or privileged
seat/machine acceptance. See [polkit setup](../polkit.md).
