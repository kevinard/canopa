# 0125: Shared authentication input and accessibility

Status: accepted

`canopa-session-ui` owns bounded UTF-8 editing, logical grapheme cursor/selection,
software shaping/font fallback and passive AT-SPI semantics. Lock, Greeter, Polkit
and Pinentry retain their authentication state machines, surface roles and action
validation. Incoming accessibility actions carry a presentation revision and are
revalidated by the consumer before invoking the same input/authentication actions.
Hidden responses never enter a published accessibility tree. Secret text supplied
by an accessibility peer is erased after transfer into owned bounded storage.

Owned secrets use dedicated anonymous mappings, best-effort mlock and
MADV_DONTDUMP. Dedicated mappings avoid unlocking an unrelated allocator object's
shared page. Erasure covers deleted ranges, the unused tail and mapping teardown.
PAM, transport and library-owned copies remain outside that ownership guarantee.

The local keyboard selector uses only compositor-supplied XKB groups. F6 and the
Keyboard control change local interpretation; a compositor group change takes
precedence. Selected groups retain modifier and compose handling. This does not
install or rewrite a compositor's keyboard configuration.

Cosmic Text supplies advanced shaping/bidirectional layout and Swash rasterization
on the CPU. No GPU device or animation/redraw timer is introduced. Consumers still
control redraws and widget lifetimes. Native acceptance uses independent libatspi,
Wayland key injection and PAM/greetd/polkit/Assuan peers. See the
[input guide](../session-tools.md#shared-authentication-input).
