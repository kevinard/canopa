# Mutation commitment and effects

Notification History writes return their exact revision using SQLite `RETURNING`, with decoding and retention pruning inside the same transaction. A failure rolls back the mutation; a successful receipt cannot accidentally describe a subsequent concurrent revision. This deepens ADR 0012's storage boundary.

The Notification Runtime serializes lifecycle mutations and effect publication through one mutation coordinator. Activation-token acquisition has a bounded wait outside that coordinator; invocation revalidates the observed revision afterward. History preconditions retain the revision originally validated by timeout/action handlers.

Banner interactions capture a `BannerTarget` (thread ID and revision) when the widget is built, including dismissal and default activation. The target survives contextual activation-token callbacks and is checked again by the runtime; a stale target is rejected without acting on the replacement, including when an action key is reused. Current-state control commands still resolve their target when handled, but default-action selection and commitment use the same observed revision. This is Canopa's explicit stale-interaction policy, not an additional wire-protocol requirement.

[ADR 0042](0042-revision-bound-notification-center-actions.md) extends captured-revision safety to Notification Center controls, also binding their targets to the producing daemon lifetime. Ordinary current-state commands remain unchanged.

Bulk clear is explicitly **partial-success** across durable and transient threads. It stops on the first failed transition, publishes all earlier committed transitions, and reports the failure and committed-dismissal count. Remaining threads stay active; repeating clear retries them. The library's `LifecycleBatch` carries effects independently from the operation result, so an error cannot discard committed effects.

The Control API returns `ClearPartiallyApplied { dismissed }` for this outcome. JSON socket clients receive that typed result; `ctl clear` exits unsuccessfully with the count. Legacy panel interaction replies report the same count without exposing raw storage errors.

Banner rendering is a replaceable projection, not part of storage commitment. Renderer failure is exposed as presentation health and must not convert a committed receipt or closure into an ambiguous failed request. Sender events require ordered publication independent of replaceable presentation; diagnostic subscriptions are not the transport's delivery guarantee.

The iced transport holds one replaceable pending projection instead of a FIFO that can reject the final update during a burst. Each consumed projection carries its own display acknowledgement. Skipped projections must not start timers; an actual frame from an older projection may still acknowledge that frame, with runtime revision/cancellation checks preventing obsolete timers. No capability advertisement or graphical startup-readiness claim follows from this transport change.

Renderer retirement and publication share a send/close gate. Closing the gate is terminal and releases pending acknowledgement callbacks before the unclaimed receiver is removed; later publication fails without retaining another callback-owned runtime. This also applies when renderer startup exits before taking its subscription. Disconnection is reported on the next attempted presentation update; proactive renderer health and automatic restart recovery are separate work.
