# Mutation commitment and effects

Notification History writes return their exact revision using SQLite `RETURNING`, with decoding and retention pruning inside the same transaction. A failure rolls back the mutation; a successful receipt cannot accidentally describe a subsequent concurrent revision. This deepens ADR 0012's storage boundary.

The Notification Runtime serializes lifecycle mutations and effect publication through one mutation coordinator. Activation-token acquisition has a bounded wait outside that coordinator; invocation revalidates the observed revision afterward. History preconditions retain the revision originally validated by timeout/action handlers.

Bulk clear is explicitly **partial-success** across durable and transient threads. It stops on the first failed transition, publishes all earlier committed transitions, and reports the failure and committed-dismissal count. Remaining threads stay active; repeating clear retries them. The library's `LifecycleBatch` carries effects independently from the operation result, so an error cannot discard committed effects.

The Control API returns `ClearPartiallyApplied { dismissed }` for this outcome. JSON socket clients receive that typed result; `ctl clear` exits unsuccessfully with the count. Legacy panel interaction replies report the same count without exposing raw storage errors.

Banner rendering is a replaceable projection, not part of storage commitment. Renderer failure is exposed as presentation health and must not convert a committed receipt or closure into an ambiguous failed request. Sender events require ordered publication independent of replaceable presentation; diagnostic subscriptions are not the transport's delivery guarantee.
