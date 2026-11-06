# Prepared capture-bound pointer contract

OpenMappedPointer on org.canopa.Niri.RemoteInput1 takes one Mutter capture
session object path and 1–16 unique stream object paths from that session.
MappedPointerVersion=1 negotiates this additional endpoint. It returns a lease
FD, capability mask 31, and (stream ID, mapping ID, logical size) descriptors.
The compositor still delegates portal authorization and consent to Canopa.

The existing OpenPointer/PointerVersion=1 returns mask 15. Its behavior is
unchanged; an absolute command on that unbound endpoint retires the lease.
Both endpoint types share the eight-slot input admission owner.

The command framing and acknowledgement format remain 32/16 bytes. Kind 5 uses
two nonnegative finite f64 coordinate words and the nonzero u64 stream identity.
Points must lie within the immutable logical size and the stream must belong to
this binding. Invalid commands retire the entire source without acknowledging.

Selection pins alone do not authorize mapping. Every region requires the exact
selected session/stream, a fixed source, and an extant PipeWire producer with its
node established. Recycled node numbers or connector names do not suffice.
The compositor loop checks source lifetime and region dimensions before commands
and during idle refresh. Session stop, source loss, resize, lock or FD/I/O loss
retires the source, including its held buttons and owned grabs.

Monitor points add the current global output origin to stream logical coordinates;
hardware output transforms are not applied again. Window points follow the
current rendered root-surface origin, including tile offsets and workspace
translation. Their video bbox uses the same physical rounding as capture.
The current implementation rejects overview and points outside the selected
window’s actual input region or obscured by another surface. It does not
activate an inactive workspace or provide complete window-only remote control.
That limitation remains before portal advertisement.

This is prepared compositor code, not canonical adoption. The existing public
portal, EI worker, legacy Notify adapter, Iced checkbox and shipped descriptors
have not gained pointer permission from this slice.
