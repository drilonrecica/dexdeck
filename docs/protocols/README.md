# DexDeck protocols

DexDeck exposes versioned contracts from the first release:

- [CLI schema v1](cli-v1.md) for JSON snapshots and JSONL streams
- [Gradle bridge protocol v1](bridge-v1.md) for model discovery

Changing a field name, type, meaning, required status, or enum value requires a
compatibility test and an explicit schema-version decision. Human output is not
a stable machine interface.
