# event-ledger

A small Rust trading-event ledger built to explore the core mechanics behind durable event processing systems: normalization, validation, append-only persistence, replay, deduplication, and reconstruction of derived state.

The project is intentionally small. The goal is not to build a production database, but to make the failure modes and ordering guarantees of a trading-platform event pipeline explicit.

## Architecture

```text
External message
      ↓
   Ingest
      ↓
Normalization
      ↓
 Validation
      ↓
Deduplication
      ↓
Append-only ledger
      ↓
   Replay
      ↓
Derived state
```

The ledger acts as the authoritative history. Derived state such as positions can be reconstructed by replaying persisted events after a restart.

## Event model

External exchange messages are converted into a common internal representation.

```rust
Event {
    source,
    source_id,
    timestamp,
    kind,
}
```

A fill contains:

```rust
Fill {
    instrument,
    side,
    quantity,
    price,
}
```

The separation between external input and the internal domain model prevents exchange-specific representations from leaking throughout the rest of the system.

For example, an external side value such as `"BUY"` is normalized into:

```rust
EventSide::Buy
```

Malformed representations fail normalization, while semantically invalid values such as negative quantities fail validation.

## Ledger format

Each event is serialized as JSON and written as a framed binary record.

```text
┌────────────── 24-byte header ──────────────┐
│ magic       4 bytes                        │
│ version     4 bytes                        │
│ sequence    8 bytes                        │
│ payload_len 4 bytes                        │
│ checksum    4 bytes                        │
├────────────────────────────────────────────┤
│ serialized Event payload                   │
└────────────────────────────────────────────┘
```

Header layout:

| Offset | Field | Type |
|---|---|---|
| `0..4` | Magic | `[u8; 4]` |
| `4..8` | Version | `u32` |
| `8..16` | Sequence | `u64` |
| `16..20` | Payload length | `u32` |
| `20..24` | CRC32 | `u32` |

All numeric fields are encoded using little-endian byte order.

The current magic value is:

```text
EVLG
```

and the current format version is `1`.

## Why framing?

Writing debug strings or newline-separated data is simple, but it does not provide strong record boundaries or corruption detection.

A framed record allows recovery code to determine:

- where a record starts;
- which version of the format it uses;
- its ledger ordering;
- exactly how many payload bytes must follow;
- whether the payload was corrupted or partially written.

The checksum is calculated over the serialized payload.

## Append path

Appending an event performs:

```text
Event
  ↓
serialize to bytes
  ↓
calculate payload length
  ↓
calculate CRC32(payload)
  ↓
build header
  ↓
write header
  ↓
write payload
  ↓
increment sequence
```

The sequence is incremented only after both writes succeed.

`write_all` is used instead of `write` so partial writes are handled by the standard library rather than being silently treated as complete records.

## Recovery

When an existing ledger is opened, records are scanned from the beginning.

For each record:

```text
probe first byte
      ↓
read complete header
      ↓
validate magic
      ↓
validate version
      ↓
validate sequence
      ↓
validate payload length
      ↓
read exact payload
      ↓
verify CRC32
      ↓
deserialize Event
```

A one-byte probe is used before reading the rest of the header so recovery can distinguish between:

```text
clean EOF
```

and:

```text
a record started but its header was truncated
```

After scanning all valid records, `next_sequence` is restored so appends continue from the correct ledger position.

## Replay

The ledger can rewind its read cursor to the beginning of the file and reconstruct all persisted events:

```rust
ledger.replay()
```

Replay performs the same record validation as startup recovery but keeps the deserialized events in a `Vec<Event>`.

This provides the basis for rebuilding derived state after process restart.

## Position reconstruction

A `PositionState` applies replayed fills to a per-instrument position map.

Conceptually:

```text
BUY  BTCUSDT 0.5  → +0.5
SELL BTCUSDT 0.2  → +0.3
```

Because positions are derived from event history rather than treated as the source of truth, they can be reconstructed after restart.

## Deduplication

External events are identified using:

```text
(Source, source_id)
```

A `HashSet` tracks event identities already persisted.

At startup, the deduplicator is rebuilt from replayed events.

For a new event:

```text
already seen?
    yes → ignore

    no
    ↓
append successfully?
    no  → leave unseen
    yes → mark as seen
```

The identity is only inserted into the deduplication set after persistence succeeds. This avoids losing an event when an append fails after the duplicate check.

## Current guarantees

The project currently demonstrates:

- source-specific ingestion;
- normalization into a common event model;
- semantic validation;
- append-only persistence;
- explicit binary record framing;
- monotonically increasing ledger sequences;
- payload length validation;
- CRC32 integrity checking;
- startup scanning;
- replay after restart;
- event deduplication;
- reconstruction of derived position state.

## Deliberate simplifications

This is an educational systems project rather than a production ledger.

Current simplifications include:

- JSON payload encoding rather than a compact binary schema;
- `f64` for price and quantity rather than fixed-point decimal representation;
- a single-process writer;
- no indexing;
- no batching;
- no asynchronous ingestion;
- no replication;
- no snapshots;
- no schema migration beyond a format version field.

A production trading system would need stronger durability, concurrency, observability, and operational guarantees.

## Durability caveat

Successful `write_all` calls do not necessarily mean data has reached durable storage.

The current append path means:

```text
userspace → kernel/page cache
```

rather than necessarily:

```text
userspace → kernel → physical storage
```

A stronger durability policy could explicitly call `sync_data()` or `sync_all()` depending on the required latency/durability trade-off.

## Failure handling

The record structure is designed so a crash during an append can be distinguished from a clean end of file.

The intended recovery policy is:

```text
clean EOF
→ valid end of ledger

partial final header
→ treat as torn final record

partial final payload
→ treat as torn final record

invalid magic/version/sequence
→ corruption

CRC mismatch
→ corruption
```

A natural extension is to truncate an incomplete final record back to the end of the last fully validated record.

## Project structure

```text
src/
├── main.rs
├── domain.rs
├── ingest.rs
├── normalize.rs
├── validation.rs
├── ledger.rs
├── dedupe.rs
└── positions.rs
```

Responsibilities:

```text
domain.rs
    Internal event types.

ingest.rs
    Exchange/source-specific input types.

normalize.rs
    Converts external messages into internal Events.

validation.rs
    Enforces semantic invariants.

ledger.rs
    Persistence, framing, recovery, and replay.

dedupe.rs
    Tracks already persisted external event identities.

positions.rs
    Reconstructs derived positions from event history.

main.rs
    Wires the pipeline together.
```

## Running

```bash
cargo run
```

To start with a fresh ledger:

```bash
rm -f ledger.txt
cargo run
```

Running the program again demonstrates restart recovery and replay.

For the current synthetic Binance fill, the first run persists the event. On the following run, the event is replayed to reconstruct the position and the repeated exchange trade ID is recognized as a duplicate.

## What I learned

This project focuses on a few systems concepts that are easy to hide behind higher-level storage abstractions:

- durable state is different from in-memory state;
- event ordering needs an explicit definition;
- record boundaries matter during crash recovery;
- successful writes are not automatically durable writes;
- checksums detect corruption but do not themselves repair it;
- replay allows derived state to be rebuilt from authoritative history;
- idempotency is essential when external systems may redeliver events;
- persistence should happen before an event is considered committed internally;
- file APIs are stateful: cursor position affects subsequent reads;
- binary formats require explicit decisions about layout, endianness, versioning, and length limits.

The resulting system is deliberately small enough that each of those decisions remains visible in the code.