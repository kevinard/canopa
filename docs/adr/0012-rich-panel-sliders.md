# 0012: Optional bounded sliders in rich panel items

Status: accepted (2026-09-30).

Sound needs an absolute volume control rather than repeated actions. Rich items
may now carry one optional `slider` with an opaque key, label, unit, minimum,
maximum, step and current value. Numeric fields are unsigned 16-bit integers;
validation requires a nonempty range, nonzero step, aligned endpoint/current value,
and a key distinct from the item's buttons and body links. A slider counts as one
interactive control against the existing content limit.

This extends the shared rich primitive used by v4 and v5 without changing framing
or adding a new interaction variant. An item `Invoke` carries the advertised slider
key and a canonical decimal value string. The v5 session and host reject values
outside the declared range/grid, unknown controls and stale revisions/generations.
Omitting `slider` omits it from serialization and preserves existing item bytes.
Older rich readers ignore this optional field; provider and host should be updated
together to display the control. v1 is unchanged. This extends ADRs 0006 and 0007.

The host owns the widget, styling, Tab focus and native slider semantics. Pointer
dragging and held arrow keys preview in widget state; release commits the exact
final value once. Home/End select endpoints; native accessibility supports bounded
set-value, increment and decrement. Preview state belongs to a captured generation:
a replacement cancels an unfinished gesture rather than rebasing it onto a new
contract. Focus can still follow the same opaque control after replacement.
Focus reveal compares the logical provider/item/control identity so replacements
preserve scroll position. A held key's repeats cannot start a replacement gesture;
it requires a fresh press. Consecutive commits compare with the last submitted
value, including a return to the previously advertised value.
Providers retain their domain validation and error/recovery behavior.

Sound replaces its two volume buttons with a 0–100% slider in one-point steps.
The displayed level now represents the loudest channel, matching the absolute
control. Pulse channel scaling preserves existing nonzero channel proportions and
leaves mute unchanged. Values amplified externally appear at the slider's 100%
limit; merely opening the panel does not lower them. Muting and choosing the default
remain separate actions. The normal updater installs both host and provider.
The provider validates revisions when receiving interactions, then serializes
accepted commands in a bounded queue. Adjacent absolute volume edits for the same
device coalesce to the latest value; mute/default and other devices retain order.
The audio worker revalidates captured device identity before executing each command.
