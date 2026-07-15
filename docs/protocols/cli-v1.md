# CLI schema version 1

Snapshot commands write one JSON object containing schemaVersion and data.
Streaming commands write one compact JSON object per line; progress or human
diagnostics never share structured stdout.

All field names use camelCase. Unknown event types must not be silently treated
as successful operations. Paths are emitted as platform-native Unicode strings.
Secrets are never valid protocol fields.

The stable event families are jobStarted, jobProgress, output, diagnostic,
testResult, log, jobFinished, and error.
