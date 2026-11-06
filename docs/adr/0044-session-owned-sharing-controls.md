# 0044: Session-owned screen-sharing controls

Status: implemented; private native acceptance passes, 2026-10-30.

An active ScreenCast session owns an Iced control window as well as its recording.
The short-lived Start request owns neither lifetime after returning its result.
Create the controls after producer/serial validation and require a bounded ready
acknowledgment before returning streams. Failure to open the controls stops the
recording instead of granting an invisible session.

The window shows the requesting application and selected source labels with the
shared appearance and a Stop sharing action. It is an ordinary unparented Canopa
toplevel with a fixed 480×280 size so Niri initially floats it; it does not retain
the consent window's foreign parent or activation
token. This initial surface does not imply a compositor-wide overlay or panel
integration. Each session has its own controls, and source discovery excludes
Portal windows.

Stop, Escape, window closure, helper exit and malformed helper output revoke the
session. The session actor then emits Closed and stops its actual recording.
Frontend Session.Close, source loss, caller loss and backend shutdown retire both
resources. Start-request completion or loss of its cancellation sender does not
close active controls. Request cancellation during startup still aborts them.

Keep the helper process and its stdin lease in a detached bounded driver. Dropping
the owner closes that lease; EOF exits Iced. Publish revocation before waiting for
UI cleanup, and request capture Stop concurrently with control retirement. Allow
three seconds for the helper to exit, then kill and reap it. A dropped opening
future must retain this cleanup. The capture worker remains separately owned as
specified in ADR0041, including cleanup after backend death.

Use a private framed input of at most 16 KiB, at most 16 bounded sanitized source
labels, and a strict ready acknowledgment of at most 256 bytes within ten seconds.
Any subsequent output or EOF makes the controls unavailable. Do not put private
source labels in cleanup logs. Blocking stdin observation runs on a reader thread;
the default Iced executor does not provide a Tokio runtime.

Six process tests cover readiness, cancellation, completed-request independence,
source closure, explicit stop/drop, child failure, and forced retirement. Seventeen
private native groups pass in dark/100% and light/125% on the ADR0043 compositor
candidate. They verify initial floating placement, actual producer removal for
Stop, Escape and window closure, retirement on frontend Close/source loss/active
caller disconnect, and worker cleanup after backend SIGKILL. ScreenCast remains
unadvertised pending compatible compositor distribution selection. Runtime cursor
capability gating and all cursor-mode native checks also pass.
