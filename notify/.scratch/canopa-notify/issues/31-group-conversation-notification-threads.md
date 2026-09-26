# Group conversation Notification Threads end to end

Status: ready-for-agent
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Implement the chosen conversation identity through Notification Server Request decoding, Notification Intake, Notification History, Notification Center projection, panel protocol, and interactive grouped presentation. Individual Notification Threads must retain their own lifecycle and actions while related threads can be scanned and controlled as one conversation group.

## Acceptance criteria

- [ ] Supported transports decode and validate the agreed conversation identity without changing App Identity or replacement semantics.
- [ ] Notification History persists the conversation identity and reads existing rows without one.
- [ ] Notification Center groups first by App Identity and then presents related conversation items according to the ADR.
- [ ] A collapsed conversation exposes an accurate unread or active count and a useful latest-item summary without hiding critical actions.
- [ ] Expanding a conversation exposes individual active and historical Notification Threads with their current actions and statuses.
- [ ] Replacements update one Notification Thread in place and never create duplicate conversation items.
- [ ] Panel protocol, subscription, persistence, replacement, action, and mixed grouped/ungrouped tests cover the complete behavior.

## Blocked by

- [30-decide-conversation-thread-identity.md](30-decide-conversation-thread-identity.md)
