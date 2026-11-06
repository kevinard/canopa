# Draft compositor pointer lease

Status: unadopted Rust transport, 2026-10-31. This extends the prepared pointer
ownership foundation; the canonical vendor recipe, portal worker, Iced consent,
main registration, shipped descriptor and installed services are unchanged by
this slice. Absolute mapping and portal pointer integration remain work to do.

The five-file Niri delta adds `OpenPointer` and `PointerVersion=1` on the existing
private RemoteInput1 service. The existing Version, OpenKeyboard and
OpenKeyboardSymbols interfaces stay intact. The returned FD is the lease; its
capability mask15 covers relative motion, buttons, continuous/finger scroll and
wheel v120 input. Absolute and touch capabilities are absent. This compositor API
does not make portal consent decisions; the trusted portal still owns grants.

Commands are32bytes: little-endian u32 sequence/kind and three u64 payload words.
Sequences start at1 and never wrap. Replies contain the sequence and three zero
u32 words. The wire module documents barrier, relative, button, scroll and wheel
layouts. It rejects invalid kinds, flags, codes, states, nonfinite/excessive
amounts, sequence errors and truncated EOF. Each source permits64held buttons;
commands are read without read-ahead, at most32per dispatch. Wheel v120 magnitudes
are bounded before Niri's user configuration is applied. Keyboard and pointer
leases share one eight-slot admission owner.

The normal input handlers receive a typed RemotePointerDevice with a unique,
nonreused process-local identity. The ownership foundation handles overlapping
holds, consumed presses, bindings and grabs. FD loss and I/O failure retire the
exact source; cleanup runs before shutdown becomes observable to the peer.
Reply backpressure retires a source rather than accumulating an output queue.
Idle lock transition retires pointer leases and denies new ones. Keyboard
behavior remains covered by its existing suites; this is not a claim that the
entire portal's lock policy is complete.

Six wire tests pass. One frozen compositor passes94native checks:18lease contract
checks,8direct lease binding checks,5direct lease drag checks,38existing cleanup
checks and25existing typed/evdev keyboard checks. Its182.46-second incremental
dev build and77.96-second native run preserve all5,957prepared source hashes and
the recorded runtime/recipe/fixture/binary pins. `draft.json` identifies exact
hashes and `target/remote-input-pointer-lease-03/` retains evidence and failed
controls. Synthetic/nested input does not establish physical hardware acceptance.

The layered delta replay verifies11changed/added files against the preserved
canonical base, ownership patches and this lease patch. This is not a new full
archive verification, recipe adoption or full workspace/matrix/distribution gate.
The original upstream licences remain: Niri GPL-3.0-or-later and Smithay MIT.
The new backend adapter retains its derived Smithay attribution and MIT notice.

The first slow-reader fixture incorrectly drained replies while expecting
backpressure retirement. The corrected nonreading control observes held-input
cleanup before draining; queued unread commands cause a peer reset. That reset
is accepted only by the intentional backpressure case after native cleanup is
observed. Normal lease shutdown still requires EOF. Both failed preparations and
the corrected native receipts remain available.

`native-proof/` preserves direct lease test sources. `mapping-investigation.json`
records the next source-identity/geometry work. The full18-interface goal remains;
this transport alone does not grant pointer control through the portal.
