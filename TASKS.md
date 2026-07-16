# DexDeck v0.2.0 Implementation Tasks

This is the execution ledger for [SPECS.md](SPECS.md). Tasks are ordered by
dependency first, then priority and criticality.

## Execution contract

- Complete phases sequentially as continuous implementation batches.
- Keep every task as one atomic conventional commit using the listed message.
- Run the task verification before committing and keep the worktree clean.
- P0: release blocker. P1: required capability. P2: launch quality.
- Critical: correctness/privacy/security. High: acceptance feature. Medium: polish.
- Do not rewrite historical DroidDeck tags or introduce retired identifiers.
- Do not continue past a failed phase gate.

## Locked decisions

- Release: DexDeck v0.2.0; Apache-2.0; Copyright 2026 Drilon Recica.
- Rust: 1.97.0, edition 2024, resolver 2; internal crates remain private.
- Full model support: AGP 8.x and 9.x through separate adapters. Other versions
  use explicit degraded mode.
- Bridge: Java 17, reproducible tracked JAR, embedded and hash-verified.
- Public CLI, bridge, config, cache, test, and diagnostic schemas start at v1.
- JSON uses camelCase. TOML uses snake_case.
- Snapshot output: human or JSON. Streaming output: human or JSONL.
- Stable targets: macOS ARM64/x86-64 and Linux glibc ARM64/x86-64.
- Experimental target: Windows x86-64.
- Distribution: GitHub archives, checksums, POSIX/PowerShell installers, and
  drilonrecica/tap. Crates.io and the standalone website are deferred.

## Phase 0 — Authoritative baseline and repository foundation

### F0.1 Reconcile the authoritative specification — P0/Critical

- Move the specification to canonical root SPECS.md.
- Record v0.2.0, preserved historical tags, AGP 8–9 range, and crates.io deferral.
- Verify links and retired-identifier usage with rg.
- Commit: "docs(spec): align DexDeck 0.2 release scope"

### F0.2 Record foundational ADRs — P0/Critical

- Add an ADR index and the eleven decisions required by SPECS.md.
- Each ADR records context, decision, consequences, rejected options, and status.
- Verify all decisions are accepted, linked, and mutually consistent.
- Commit: "docs(architecture): record foundational decisions"

### F0.3 Establish the Cargo workspace — P0/Critical

- Add the eight specified private crates, shared dependency/lint configuration,
  Rust 1.97.0 pin, release profiles, Cargo.lock, and centralized brand constants.
- Set dexdeck to 0.2.0 and publish=false on internal crates.
- Verify cargo check --workspace --all-targets.
- Commit: "chore(workspace): scaffold DexDeck crates"

### F0.4 Add licensing and governance — P1/High

- Add LICENSE, NOTICE, CONTRIBUTING.md, SECURITY.md, DCO instructions, issue
  templates, and pull-request template. Do not add a Code of Conduct.
- Verify package and document metadata.
- Commit: "docs(governance): add project policies"

### F0.5 Add baseline CI — P0/Critical

- Add SHA-pinned GitHub Actions for fmt, Clippy with warnings denied, tests,
  docs, DCO, and Linux/macOS/Windows compilation.
- Verify CI never rewrites tracked files.
- Commit: "ci: add baseline workspace checks"

### F0.6 Enforce dependency privacy policy — P0/Critical

- Add advisory, license, source, duplicate, and banned-dependency checks.
- Ban product HTTP clients and require review for socket/TLS-capable dependencies.
- Commit: "ci: enforce dependency privacy policy"

**Phase gate:** clean checkout passes formatting, linting, tests, dependency
policy, and cross-platform compilation.

## Phase 1 — Protocols, runtime, configuration, CLI, and terminal safety

### F1.1 Define versioned data contracts — P0/Critical

- Define normalized project/module/variant/task/artifact, diagnostic, test,
  job, bridge-record, CLI snapshot, and streaming-event types.
- Require schema version on every public record and golden-test JSON/JSONL.
- Commit: "feat(protocol): define versioned data contracts"

### F1.2 Implement secure local storage — P0/Critical

- Resolve platform paths and project hashes; add restrictive permissions,
  versioned envelopes, fsync, atomic replace, and corrupt-file recovery.
- Test symlinks, permissions, interrupted writes, and corrupt input.
- Commit: "feat(config): add secure local storage"

### F1.3 Implement layered configuration — P0/High

- Add schema-v1 project, Gradle, UI, Logcat, editor, profile, environment
  reference, and custom-command configuration.
- Implement CLI/user/shared/detected/default precedence, comment-preserving TOML,
  precise diagnostics, backups, and explicit shared migrations.
- Reject shell strings and persisted secret values.
- Commit: "feat(config): implement layered configuration"

### F1.4 Add errors and secret redaction — P0/Critical

- Implement the complete error taxonomy with operation context and recovery.
- Make secrets non-displayable and redact environment, token, password, and
  signing-like values from all outputs.
- Add property tests for redaction.
- Commit: "feat(core): add errors and secret redaction"

### F1.5 Implement state and effect runtime — P0/Critical

- Add deterministic reducers, AppState, named actions, effect requests, bounded
  channels, cancellation tokens, and injectable time/ID sources.
- Test replay, backpressure, cancellation, and reducer purity.
- Commit: "feat(core): implement state and effect runtime"

### F1.6 Add supervised job scheduling — P0/Critical

- Implement job states, one mutating Gradle job per root, concurrent non-Gradle
  jobs, queueing, byte-bounded output, diagnostics, and fifty persisted records.
- Commit: "feat(core): add supervised job scheduling"

### F1.7 Supervise child process trees — P0/Critical

- Add direct-argv CommandSpec, controlled environment/cwd, bounded stdio,
  Unix process groups, and Windows Job Objects.
- Gracefully interrupt first, then kill the full tree on timeout/second cancel.
- Test with grandchildren and signal-resistant helpers.
- Commit: "feat(core): supervise child process trees"

### F1.8 Define the complete CLI surface — P0/High

- Add all specified commands plus clean-reinstall and command run.
- Implement global options, terminal detection, format validation, separated
  stdout/stderr, stable exit codes, help, and versioned version output.
- Commit: "feat(cli): define DexDeck command surface"

### F1.9 Add the event-driven terminal shell — P0/Critical

- Implement the terminal RAII guard, panic restoration, raw/alternate screen,
  input/resize handling, event-driven rendering, semantic Lazuli tokens,
  Unicode/ASCII/no-color modes, and minimum-size fallback.
- Verify clean exit and panic restoration under a pseudo-terminal.
- Commit: "feat(tui): add event-driven terminal shell"

### F1.10 Add opt-in debug diagnostics — P1/High

- Add in-memory diagnostics and explicit redacted --debug-log output.
- Persist no application log or crash report by default.
- Commit: "feat(core): add opt-in debug diagnostics"

**Phase gate:** TUI restores the terminal; reducers/effects are tested; process
tree cancellation works; CLI emits valid schema-v1 output.

## Phase 2 — Android project discovery and Gradle modeling

### F2.1 Create the Android fixture matrix — P0/High

- Add minimal Kotlin/Groovy, single/multi-module, multi-app, flavors, disabled
  variants, libraries, convention plugins, buildSrc, composites, custom tasks,
  broken/missing-wrapper, and AGP 7 degraded fixtures.
- Add AGP 8.0.2, 8.13, 9.0.1, and 9.3.0 compatibility lanes.
- Commit: "test(fixtures): add Android project matrix"

### F2.2 Implement fast filesystem discovery — P0/High

- Walk upward or honor --project; detect root, wrapper, settings, Android
  signals, and basic SDK/JDK signals without invoking Gradle.
- Test nested roots, symlinks, both DSLs, ambiguity, and non-Android builds.
- Commit: "feat(gradle): detect Android project roots"

### F2.3 Build the Java bridge framework — P0/Critical

- Add Java 17 build, init plugin, model task, adapter interface, version
  detection, explicit JSONL output, structured failures, and completion sentinel.
- Commit a reproducible JAR/hash and make CI byte-compare rebuilt output.
- Commit: "feat(gradle): add versioned model bridge"

### F2.4 Implement the AGP 8 adapter — P0/Critical

- Use public Android Components APIs to model modules, dimensions, flavors,
  build types, variants, IDs, SDKs, tests, tasks, artifacts, and included builds.
- Verify minimum and upper AGP 8 fixtures.
- Commit: "feat(gradle): model AGP 8 projects"

### F2.5 Implement the AGP 9 adapter — P0/Critical

- Use AGP 9 public DSL/Variant APIs without removed legacy interfaces.
- Produce the same normalized model and test 9.0 plus current 9.3 behavior.
- Commit: "feat(gradle): model AGP 9 projects"

### F2.6 Embed and invoke the bridge — P0/Critical

- Extract to a content-addressed cache atomically, verify hashes, prefer project
  wrappers, require approval for system Gradle, and separate stdout from JSONL.
- Reject partial output after failure/cancellation.
- Commit: "feat(gradle): embed and invoke the bridge"

### F2.7 Orchestrate project model refresh — P0/High

- Implement provisional discovery, cache load, async validation, Gradle refresh,
  normalization, cancellation, and explicit freshness states.
- Preserve the previous snapshot when refresh fails.
- Commit: "feat(gradle): orchestrate project model refresh"

### F2.8 Cache and fingerprint models — P0/Critical

- Persist versioned model/fingerprint files atomically.
- Fingerprint only model inputs using metadata first and content hashes for
  changed files; never scan arbitrary source trees.
- Commit: "feat(config): cache and fingerprint project models"

### F2.9 Watch model inputs — P1/High

- Add cross-platform event watching, debounce, immediate stale status, delayed
  refresh during Gradle work, and valid session selection restoration.
- Commit: "feat(config): watch project model inputs"

### F2.10 Add explicit degraded mode — P0/High

- Handle AGP outside 8–9, bridge incompatibility, unavailable APIs, missing
  wrappers, and configuration failures without claiming full support.
- Preserve usable cache and allow tasks, manual profiles, ADB, and Logcat.
- Commit: "feat(gradle): support explicit degraded mode"

### F2.11 Expose project model commands — P1/High

- Implement project inspect, modules list, and variants list with deterministic
  human/JSON output, freshness, and degraded status.
- Commit: "feat(cli): expose project model commands"

### F2.12 Verify the modeling matrix — P0/Critical

- Test all fixtures, protocol compatibility, corrupt/partial output, cache
  invalidation, bridge cancellation, and unchanged project Git status.
- Commit: "test(gradle): verify project modeling matrix"

**Phase gate:** supported fixtures model correctly; current cache avoids Gradle;
changes become stale; failure never replaces a valid snapshot.

## Phase 3 — SDK, devices, emulators, Gradle tasks, and app execution

### F3.1 Resolve Android SDK tools — P0/High ✅

- Implement specified SDK/ADB/emulator/sdkmanager resolution and a doctor model.
- Diagnose missing packages and print commands without silent installation.
- Commit: "feat(android): resolve SDK tools"

### F3.2 Track ADB devices — P0/Critical ✅

- Start ADB lazily; consume track-devices with restart/backoff; enrich model,
  product, API, transport, classification, and authorization state.
- Require explicit or valid restored active-device selection.
- Commit: "feat(android): track ADB devices"

### F3.3 Manage existing emulators — P1/High ✅

- List, inspect, start, cold boot, confirmed wipe, stop, map serials, and monitor
  boot completion. Never create or implicitly start/stop AVDs.
- Commit: "feat(android): manage existing emulators"

### F3.4 Execute queued Gradle tasks — P0/Critical ✅

- Implement wrapper task execution, task metadata/search/recent use, layered
  arguments, protected internal flags, and scheduler integration.
- Commit: "feat(gradle): execute queued Gradle tasks"

### F3.5 Resolve run profiles — P1/High ✅

- Resolve module, variant, device, launch intent, Gradle properties, and secret
  environment references with validation and confirmation rules.
- Commit: "feat(core): resolve run profiles"

### F3.6 Implement app lifecycle commands — P0/Critical ✅

- Add assemble/install selection, artifact discovery, split APK support, package
  and component resolution, deep links, force-stop, uninstall, and clear data.
- Preserve defaults: replace enabled; downgrade/grant-all/uninstall-first disabled.
- Commit: "feat(android): implement app lifecycle commands"

### F3.7 Compose explicit run workflows — P0/High

- Implement Build, Install, Launch, Run, Rerun, Reinstall, Clean Reinstall, and
  Stop exactly as specified, with cancellation and destructive confirmations.
- Commit: "feat(core): compose explicit run workflows"

### F3.8 Guard custom commands — P0/Critical

- Execute argv-only commands with bounded output and validated cwd.
- Add trust once/project/cancel, show argv/cwd, and invalidate on remote change.
- Commit: "feat(core): guard trusted custom commands"

### F3.9 Expose Android CLI operations — P1/High

- Implement doctor, device/emulator, build/run lifecycle, Gradle, and custom
  command handlers with human/JSON/JSONL and stable errors.
- Commit: "feat(cli): expose Android operations"

### F3.10 Test execution workflows — P0/Critical

- Use fake SDK/ADB/emulator/Gradle tools to cover failures, disconnects,
  unauthorized devices, cancellation, release/destructive confirmation, split
  artifacts, data preservation, and orphan prevention.
- Commit: "test(android): cover execution workflows"

**Phase gate:** selected module/variant/device can build, install, launch, and
stop; normal workflows preserve data; cancellation leaves no process behind.

## Phase 4 — Structured bounded Logcat

### F4.1 Parse structured log streams — P0/Critical

- Incrementally parse timestamps, PID/TID/UID, priority, tag, message, process,
  continuations, Java/native crashes, and best-effort ANR markers.
- Fuzz partial, malformed, invalid UTF-8, oversized, and restart inputs.
- Commit: "feat(logcat): parse structured log streams"

### F4.2 Add bounded log storage — P0/Critical

- Enforce 8 MiB minimum, 32 MiB default, 1 GiB maximum, and warning above
  256 MiB; evict complete oldest entries and track drops.
- Commit: "feat(logcat): add bounded log storage"

### F4.3 Track application processes — P0/Critical

- Supervise ADB capture with bounded channels/batching and follow application
  UID/PIDs, secondary processes, restarts, reconnects, and scope changes.
- Commit: "feat(logcat): track application processes"

### F4.4 Add filters and search — P1/High

- Implement priority/tag/package/process include/exclude, text/regex, case,
  crash-only, error focus, compiled caching, and local saved presets.
- Commit: "feat(logcat): add filters and search"

### F4.5 Add export and recording actions — P1/High

- Add pause, clear, scope/process selection, crash navigation, bounded explicit
  copy, visible/full export, and explicit recording start/stop.
- Persist nothing without an explicit action.
- Commit: "feat(logcat): add export and recording actions"

### F4.6 Expose structured logs through CLI — P1/High

- Implement logs with human/JSONL output, filters, scope, cancellation, export,
  and recording while keeping structured stdout clean.
- Commit: "feat(cli): expose structured logs"

### F4.7 Add the Logcat workspace — P1/High

- Add virtualized rows, follow/scroll, filter/search, process/scope selection,
  crash navigation, dropped status, and export/record indicators.
- Commit: "feat(tui): add Logcat workspace"

### F4.8 Stress the bounded pipeline — P0/Critical

- Test high volume, slow consumers, filter churn, reconnect storms, maximum
  buffers, parser throughput, latency, allocations, and input responsiveness.
- Commit: "test(logcat): stress bounded log pipeline"

**Phase gate:** high-volume logs do not freeze input; application scope follows
all processes; memory is bounded; no automatic disk writes occur.

## Phase 5 — Tests, diagnostics, and source navigation

### F5.1 Invoke Android test targets — P0/High

- Support local task/module/class/method and instrumentation
  module/package/class/method selection using standard Gradle mechanisms.
- Require an active device for instrumentation and retain arbitrary test tasks.
- Commit: "feat(test): invoke Android test targets"

### F5.2 Parse structured test results — P0/High

- Parse JUnit and instrumentation reports into counts, duration, failure, stack,
  and source data; tolerate missing/malformed/partial reports.
- Commit: "feat(test): parse structured test results"

### F5.3 Rerun failed selections — P1/High

- Add rerun all, failed, and selected class/method with job history.
- Refuse ambiguous reconstruction with a precise explanation.
- Commit: "feat(test): rerun failed test selections"

### F5.4 Normalize build diagnostics — P0/High

- Incrementally parse Kotlin, Java, resource, manifest, Gradle, ADB, test, and
  practical lint output into the normalized diagnostic schema.
- Commit: "feat(core): normalize build diagnostics"

### F5.5 Open source locations — P1/High

- Add argv templates/presets for supported editors and lexically parse VISUAL
  then EDITOR without shell execution.
- Validate placeholders and executable availability.
- Commit: "feat(core): open diagnostic source locations"

### F5.6 Expose structured test results — P1/High

- Implement test CLI selection, human/JSON/JSONL, rerun-failed, and exit status.
- Commit: "feat(cli): expose structured test results"

### F5.7 Add tests and diagnostics workspaces — P1/High

- Add hierarchy, counts, duration, failure detail, source opening, copy, raw
  output, and rerun actions using shared job/result data.
- Commit: "feat(tui): add tests and diagnostics"

### F5.8 Verify result and diagnostic flows — P0/Critical

- Add passing/failing unit/instrumentation fixtures, compiler/resource failures,
  malformed reports, editor fakes, and CLI/TUI parity assertions.
- Commit: "test(test): verify result and diagnostic flows"

**Phase gate:** supported tests run at requested granularity; failures are
structured, rerunnable, and source-addressable in CLI and TUI.

## Phase 6 — Complete operational TUI

### F6.1 Compose DexDeck services — P0/Critical

- Wire config, model, watchers, jobs, SDK, devices, emulators, execution, tests,
  diagnostics, and Logcat through the action/effect runtime.
- Centralize startup/shutdown and convert worker failures to actions.
- Commit: "feat(app): compose DexDeck services"

### F6.2 Add responsive Lazuli dashboard — P1/High

- Implement full, compact, single-workspace, and resize-warning layouts plus
  true-color, 256, 16, monochrome, and light-background adaptations.
- Never communicate status through color alone.
- Commit: "feat(tui): add responsive Lazuli dashboard"

### F6.3 Add palette and input controls — P1/High

- Add navigation, deterministic fuzzy palette, virtualized lists, help/search,
  configurable named actions, conflict detection, Vim preset, mouse, and panes.
- Commit: "feat(tui): add palette and input controls"

### F6.4 Add run and job workspaces — P1/High

- Keep project/module/variant/device/model/app/job state visible and add
  selections, explicit workflows, queue, output, diagnostics, and history.
- Commit: "feat(tui): add run and job workspaces"

### F6.5 Add device and tooling views — P1/High

- Add device selection, emulator actions, virtualized Gradle tasks, trusted
  custom commands, and actionable doctor results.
- Commit: "feat(tui): add device and tooling views"

### F6.6 Finalize session lifecycle — P0/Critical

- Restore valid selections and filters; prompt for active foreground jobs;
  never stop ADB/emulators/Gradle daemons; add reduced motion, bounded active
  animation, SSH/tmux fallbacks, and layout/event snapshots.
- Commit: "feat(tui): finalize session lifecycle"

**Phase gate:** all core operations are accessible through the TUI; remote and
compact terminals work; exit restores state without stopping external tools.

## Phase 7 — Hardening, branding, packaging, and release

### F7.1 Complete the CI matrix — P0/Critical

- Add config/protocol/cache/TUI/bridge/fixture checks and scheduled/release jobs
  for Android SDK, ADB/emulator, install/launch, instrumentation, fuzzing,
  platform smoke tests, and package installation.
- Commit: "ci: complete DexDeck validation matrix"

### F7.2 Cover critical failure modes — P0/Critical

- Test corrupt caches, permissions, interruption, watcher overflow, disconnect,
  cancellation, terminal panic restoration, active-job exit, Windows behavior,
  and unchanged project Git status.
- Commit: "test(reliability): cover critical failure modes"

### F7.3 Add reproducible benchmarks — P1/High

- Benchmark cold/warm startup, idle wakeups, input latency, model loading, task
  lists, build output, Logcat, memory bounds, and cancellation with recorded
  fixtures/hardware/tool metadata.
- Commit: "perf: add reproducible DexDeck benchmarks"

### F7.4 Enforce local-only runtime behavior — P0/Critical

- Audit telemetry, HTTP, updates, uploads, persistence, secrets, shells, trust,
  dependency trees, file permissions, and runtime socket syscalls.
- Add a concise threat model and automated privacy assertions.
- Commit: "security: enforce local-only runtime behavior"

### F7.5 Add the Deckmark asset system — P2/Medium

- Add reviewed source SVGs and deterministic PNGs for horizontal, compact,
  monochrome, and favicon forms plus terminal fallbacks.
- Validate one-color and 16px recognition, clear space, no external resources,
  no prohibited imagery, and no bundled-font requirement.
- Commit: "feat(brand): add Deckmark asset system"

### F7.6 Add user and release documentation — P1/High

- Write README, install/quick-start, CLI/config, AGP matrix, degraded mode,
  privacy, troubleshooting, known limitations, independence disclaimer, and
  authentic terminal recording; generate completions and man pages.
- Commit: "docs: add DexDeck user and release guides"

### F7.7 Configure distributions — P0/High

- Pin cargo-dist and commit reviewed workflows for all five targets.
- Rebuild/verify the bridge; produce archives, SHA-256 checksums,
  POSIX/PowerShell installers, and release provenance.
- Test archive contents, permissions, hashes, and clean-machine installation.
- Commit: "build(release): configure DexDeck distributions"

### F7.8 Add Homebrew release flow — P1/High

- Generate/test drilonrecica/tap/dexdeck against checksummed prebuilt artifacts
  and add formula installation smoke tests.
- Project commit: "build(homebrew): add DexDeck tap release flow"
- Tap commit: "feat(formula): add DexDeck 0.2.0"

### F7.9 Prepare and validate v0.2.0 — P0/Critical

- Freeze schema v1, update release notes, document Windows status, and execute
  every project/run/Logcat/test/parity/privacy/reliability/package check on clean
  supported systems.
- Commit: "chore(release): prepare DexDeck 0.2.0"
- Create tag v0.2.0 only after every gate passes.

**Final gate:** every SPECS.md acceptance criterion has an automated test or
reproducible release check; no P0/P1 task remains; stable packages install and
complete the Android daily loop; no direct network or telemetry path exists.
