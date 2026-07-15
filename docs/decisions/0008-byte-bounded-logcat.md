# ADR 0008: Byte-bounded Logcat storage

**Status:** Accepted

## Context

Log entries vary greatly in size. An entry-count limit cannot provide a useful
memory guarantee under noisy or hostile input.

## Decision

Use a byte-accounted ring buffer with a 32 MiB default, 8 MiB minimum, 1 GiB
maximum, and warning above 256 MiB. Evict oldest complete entries and expose a
dropped-entry indicator. All stream channels are bounded.

## Consequences

Memory remains predictable and high-volume input applies backpressure. Filters
and views must avoid cloning the full buffer.

## Rejected alternatives

Unbounded storage can exhaust memory. Automatic disk spooling violates privacy.
Entry-count limits do not bound bytes.
