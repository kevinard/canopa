# Notification Lifecycle Refactor

Status: ready-for-agent

## Problem Statement

The current Notification Daemon implementation mixes notification receipt, replacement handling, lifecycle transitions, history persistence, sender-visible effects, and banner runtime state in one broad path. That makes lifecycle behavior hard to test at the right interface and lets persistence details, especially banner visibility flags, leak into daemon semantics.

## Solution

Deepen **Notification Lifecycle** first. Lifecycle should own creation, revision, replacement, status transitions, timeout expiration semantics, and the domain shape of created or revised **Notification Threads**. It should use an async **Notification History** trait as a storage engine, return fine-grained **Lifecycle Effects**, and leave the **Notification Daemon** to publish sender-visible effects and presentation updates. **Notification Banner State** should consume lifecycle effects and remain runtime-only.

## Key Decisions

- **Notification Lifecycle** is separate from **Notification History**.
- **Notification Lifecycle** reads and writes **Notification History** directly through an async trait.
- **Notification History** adapters return shared domain objects, not storage records.
- **Lifecycle Effects** are fine-grained and are the primary lifecycle result.
- A **Lifecycle Effect** may include the sender receipt.
- **Notification Banner State** consumes lifecycle effects and decides eligibility itself.
- **Notification History** must not store banner runtime visibility.

