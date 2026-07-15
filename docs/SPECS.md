# DexDeck Engineering Specification

**Status:** Pre-implementation specification; product and brand direction approved  
**Product name:** DexDeck  
**Former codename:** DroidDeck; retired for all new code, packages, paths, and documentation  
**Repository:** `drilonrecica/dexdeck`  
**Implementation model:** Complete rewrite; no existing repository code or architecture is authoritative  
**License:** Apache License 2.0  
**Primary audience:** Implementers, maintainers, reviewers, designers, release engineers, and AI coding agents  
**Canonical file name:** `SPECS.md`  

---

## 1. Purpose of this document

This document is the authoritative product and engineering specification for DexDeck. It is intentionally self-contained. Implementers must not assume access to prior design discussions.

DexDeck is a high-performance, private, native terminal application for Android development. It provides a rich interactive TUI and a scriptable CLI for developers who prefer editors such as Zed, VS Code, Neovim, Vim, Helix, or a plain terminal and who do not want to keep Android Studio running for routine development work.

DexDeck is not an Android Studio clone. It is an Android development control plane focused on the daily build, run, test, device, emulator, Gradle, and Logcat loop.

The project is driven by three non-negotiable principles:

1. **Performance:** DexDeck itself must add negligible resource overhead around the Android toolchain.
2. **Privacy:** DexDeck must be fully local, contain no telemetry, make no direct network requests, and persist as little sensitive information as possible.
3. **Developer utility:** DexDeck must materially improve Android development outside Android Studio rather than merely wrap shell commands in decorative UI.

When priorities conflict, use this order:

1. Correctness and predictability
2. Privacy and local ownership of data
3. Runtime performance and resource efficiency
4. Reliability and recoverability
5. Developer experience
6. Convenience
7. Binary size

Binary size is explicitly less important than performance, reliability, and a self-contained installation.

---

## 2. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

- **MUST / MUST NOT:** Required for conformance.
- **SHOULD / SHOULD NOT:** Strong recommendation; deviation requires a documented reason.
- **MAY:** Optional.

Any implementation that violates a MUST-level privacy requirement is not DexDeck-conformant.

---

## 3. Product definition

### 3.1 One-sentence definition

DexDeck is a native Rust TUI and CLI that understands Android Gradle projects, manages variants and devices, runs builds and tests, launches applications, and provides a structured high-performance Logcat experience without requiring Android Studio.

### 3.2 Supported project family

Version 0.1 supports **native Android Gradle projects only**.

Supported source languages and build styles include:

- Kotlin Android projects
- Java Android projects
- Groovy Gradle scripts
- Kotlin Gradle scripts
- Multi-module Android repositories
- Repositories containing multiple Android application modules
- Android library modules
- Included builds and convention plugins insofar as they are required to model the primary build correctly

The following are not first-class supported project families in version 0.1:

- Flutter
- React Native
- Expo
- Unity
- Kotlin Multiplatform as a general project family
- Bazel-based Android projects
- Buck-based Android projects
- Non-Gradle Android builds

DexDeck MAY detect unsupported project families and display a precise unsupported-project explanation. It MUST NOT pretend that non-native frameworks are ordinary Android flavor configurations.

### 3.3 Android Gradle Plugin support

- Full project-model support targets **Android Gradle Plugin 8.0 and newer**.
- Older AGP projects SHOULD enter a degraded task mode instead of being rejected outright.
- Degraded mode may provide Gradle task execution, manually configured run profiles, ADB device access, and Logcat even when detailed variant modeling is unavailable.
- AGP-specific logic MUST be isolated behind versioned adapters.

### 3.4 Operating systems

Initial release targets:

- macOS ARM64: stable
- macOS x86-64: stable
- Linux x86-64 with glibc: stable
- Linux ARM64 with glibc: stable
- Windows x86-64: experimental until process, terminal, path, and Gradle wrapper behavior are proven

Later support MAY include musl/Alpine, additional architectures, and broader Windows packaging.

The implementation MUST avoid Unix-only assumptions in shared code. It MUST use direct argv-based process execution rather than shell command strings unless a future feature explicitly introduces an opt-in shell mode.

### 3.5 One project per process

A DexDeck process manages one primary Gradle root at a time.

Users who need multiple projects simultaneously should run multiple terminal tabs, windows, or tmux panes.

This simplifies:

- Active module and variant state
- Device ownership
- Logcat package filtering
- Gradle operation serialization
- File watching
- Cache identity
- Job history
- Command-palette context

### 3.6 One active device in version 0.1

Version 0.1 supports one active interactive device per DexDeck instance.

The user may switch the active device. Batch installation or testing across multiple devices is deferred.

### 3.7 Final public identity

The final public product name is **DexDeck**. The previous working name, DroidDeck, is retired and MUST NOT be introduced into new code, package names, paths, environment variables, screenshots, documentation, release artifacts, or user-visible copy.

Canonical identifiers are:

```text
Product:             DexDeck
Executable:          dexdeck
Repository:          drilonrecica/dexdeck
Rust binary package: dexdeck
Environment prefix:  DEXDECK_
Project directory:   .dexdeck/
Cache namespace:     dexdeck
Default UI theme:    Lazuli
Logo symbol name:    Deckmark
```

The name combines:

- **DEX**, the Android bytecode/runtime association
- **Deck**, a control deck, operational dashboard, or command surface

This meaning should guide product positioning without implying that DexDeck is only a DEX inspection tool. DexDeck covers the complete routine Android development loop: project understanding, variants, builds, tests, devices, application execution, Gradle tasks, diagnostics, and Logcat.

### 3.8 Positioning and approved copy

Primary product description:

> **A fast, private terminal control plane for Android development.**

Primary tagline:

> **Android development at terminal speed.**

Approved supporting descriptions include:

- Build, run, test, inspect logs, and manage Android devices without running a heavyweight IDE.
- Build Android apps without the heavyweight IDE.
- Your Android workflow. Your terminal. Nothing leaves your machine.
- A native terminal environment for the routine Android development loop.

The primary tagline is a brand statement, not a quantitative benchmark claim. Public performance claims with numbers MUST be backed by reproducible methodology, hardware details, software versions, and published benchmark code or scripts.

DexDeck MUST NOT market itself as a complete replacement for every Android Studio capability. It should state exactly what it replaces in the daily workflow and clearly disclose deferred capabilities such as visual layout editing, integrated profiling, and debugger UI.

### 3.9 Brand personality and editorial voice

DexDeck should feel:

- Precise
- Fast
- Private
- Calm
- Technical
- Independent
- Slightly opinionated
- Respectful of expert users

The voice MUST be direct and operational. It MUST NOT sound like a corporate sales page, a gaming launcher, a fan project, or a novelty terminal application.

Good operational copy:

```text
Project model refreshed in 1.3s
No device selected
Build failed in :feature:checkout
Log buffer reached 32 MiB. Oldest entries were discarded.
Configuration changed. Refreshing variants…
```

Unacceptable operational copy:

```text
Oops! Something went wrong 😭
Unleash the infinite power of Android!
Your build is cooking!
DexDeck supercharged your development experience.
```

Operational messages SHOULD NOT use emoji, jokes, hype, or anthropomorphic language. Failure messages MUST explain what failed, preserve useful context, and suggest an actionable next step where one exists.

### 3.10 Logo system: the Deckmark

The canonical logo symbol is named the **Deckmark**.

The recommended construction is two interlocking or stacked geometric `D` forms. The mark should simultaneously suggest:

- Two terminal panes
- A stacked control deck
- Layered DEX data or bytecode
- Android build variants and dimensions
- A command console
- The initials `DD`

The Deckmark MUST:

- Be based on simple filled geometry
- Work in one color
- Remain legible in monochrome
- Remain recognizable at 16×16 pixels
- Work as a favicon, GitHub avatar, Homebrew/package icon, sticker, and terminal-adjacent mark
- Avoid thin strokes that disappear at small sizes
- Avoid dependence on gradients
- Avoid dependence on color for recognition
- Have sufficient clear space around the mark

The canonical brand asset set SHOULD include:

1. Primary horizontal lockup: Deckmark plus `DexDeck`
2. Compact mark: Deckmark only
3. Monochrome light and dark versions
4. Small-size/favicon-optimized version
5. SVG source files
6. PNG exports at common avatar and documentation sizes
7. A terminal-safe textual fallback

Recommended terminal fallbacks:

```text
[DD] DexDeck
▰▱ DexDeck
```

The terminal fallback is not the canonical logo and should only be used where image assets are impossible.

### 3.11 Lazuli visual system

**Lazuli** is the name of DexDeck’s default UI theme and primary visual direction.

It is inspired by deep mineral blues, crystalline facets, angular layers, terminal panes, and precise geometric surfaces. The public identity MUST NOT advertise or depend on any Dragon Ball reference. Lazuli must stand independently as a mineral/color theme.

Canonical dark palette:

| Token | Hex | Purpose |
|---|---:|---|
| `obsidian` | `#0B1020` | Primary dark background |
| `deep_lazuli` | `#14264A` | Elevated surfaces and selected regions |
| `dex_blue` | `#3977F6` | Primary brand/action color |
| `bright_cobalt` | `#659BFF` | Focused and active elements |
| `ion_cyan` | `#59DDEA` | Secondary accent and information |
| `frost` | `#E8EFFF` | Primary text |
| `alloy` | `#96A4BC` | Secondary text |
| `graphite` | `#536078` | Borders, separators, and disabled content |
| `success` | `#45D19A` | Passed, installed, connected, healthy |
| `warning` | `#F2B85B` | Stale model, degraded mode, caution |
| `error` | `#FF667A` | Failed jobs, crashes, destructive warnings |
| `info` | `#59DDEA` | Informational status |

Implementation MUST use semantic color tokens rather than scattering literal values across widgets:

```text
color.background
color.surface
color.border
color.text.primary
color.text.muted
color.action
color.focus
color.success
color.warning
color.error
color.info
```

The exact palette MAY be tuned for terminal legibility and contrast, but changes must preserve the Lazuli identity and accessibility requirements.

DexDeck MUST support:

- True-color terminals
- 256-color approximation
- Conservative 16-color fallback
- Monochrome/no-color output
- Light-background adaptation where technically possible
- Status communication that never relies on color alone

A small amber/gold tone MAY appear as a warning or rare highlight. Blue-and-gold MUST NOT become the dominant identity because it risks a finance, cryptocurrency, or luxury-software appearance.

### 3.12 Typography

For the website, documentation, social graphics, and downloadable brand assets, the recommended family pairing is:

- **IBM Plex Sans** for headings and prose
- **IBM Plex Mono** for commands, diagnostics, metrics, and technical labels

Geist Sans and Geist Mono are acceptable alternatives if the maintainers later prefer them.

DexDeck MUST NOT bundle or require a terminal font. The TUI must work with the user’s configured monospace font and must not require Nerd Font glyphs. Nerd Font enhancements MAY be opt-in, but the default UI must remain complete with standard Unicode and ASCII fallbacks.

### 3.13 Iconography

UI icons should be geometric, minimal, and semantically consistent.

Recommended semantic forms:

```text
▶ Run
■ Stop
◆ Build
✓ Passed
✕ Failed
● Connected
○ Offline
▦ Variants
≋ Logs
```

Every icon MUST have an ASCII fallback and SHOULD have a textual label when ambiguity is possible. Emoji MUST NOT be used as operational icons.

Core visual metaphors:

- Stacked rectangles: modules, variants, and build layers
- Connected nodes: devices and processes
- Flowing horizontal lines: Logcat and streaming output
- Layered tiles: build stages
- Pulse: active operation
- Split panes: TUI and CLI over one application core

### 3.14 Motion and startup branding

Motion should communicate state, not decorate idle time.

Allowed motion includes:

- Short spinners and progress glyphs while work is active
- Brief focus transitions
- Short success/error pulses
- Subtle pane transition cues
- A compact first-launch or empty-state Deckmark animation lasting no more than approximately 250 ms

Forbidden motion includes:

- Animated backgrounds
- Continuous glow
- Fake scan lines
- Long startup sequences
- Decorative rendering while idle
- Motion that degrades SSH or multiplexer usability

Normal cached startup MUST go directly to the dashboard without a giant ASCII logo or mandatory splash screen. Perceived startup speed is part of the product identity.

Reduced-motion mode MUST remove all nonessential animation.

### 3.15 TUI brand application

Branding inside the operational TUI must be restrained.

Recommended header pattern:

```text
DEXDECK  shop-android  :app  demoDebug  Pixel_9_API_36
```

The `DEXDECK` label may use the primary action color. Project, module, variant, device, job, and model state remain dominant.

A first-launch or no-project state may use:

```text
        DEXDECK

 Android development at terminal speed.

 Detecting project…
```

A normal warm launch should not display this interstitial.

The TUI MUST prioritize information density, legibility, and fast interaction over decorative brand expression.

### 3.16 Website and README direction

Recommended website hero copy:

```text
DexDeck

Android development at terminal speed.

Build, run, test, inspect logs, and manage devices
without running a heavyweight IDE.
```

Primary calls to action:

- Install DexDeck
- View on GitHub

The website should show an authentic terminal capture or recording rather than a generic illustration.

The website and README should emphasize three pillars:

1. **Fast by construction:** native Rust application, event-driven rendering, bounded resources
2. **Fully private:** no telemetry, analytics, crash upload, update request, or direct network request
3. **Editor-independent:** compatible with Zed, VS Code, Neovim, Helix, Vim, and plain terminal workflows

Recommended README opening:

```markdown
# DexDeck

**A fast, private terminal control plane for Android development.**

DexDeck lets you build, run, test, inspect Logcat, and manage
Android devices without depending on a heavyweight IDE.
```

The README should place a small logo, concise badges, one authentic terminal recording, installation instructions, and the three product principles near the top. It should avoid excessive badges and oversized branding that pushes practical information below the fold.

### 3.17 Independent-brand and legal boundaries

DexDeck is an independent open-source project for Android development. Public documentation SHOULD include a concise statement that DexDeck is not affiliated with or endorsed by Google.

The public identity MUST NOT use:

- The Android robot or a confusingly similar robot mascot
- Android antennae or robot-head silhouettes
- Google typography, iconography, or visual-system imitation
- Android’s official green as the dominant product identity
- Dragon Ball characters, names, factions, symbols, logos, costumes, numbers, or recognizable imagery
- Red Ribbon Army imagery
- Literal playing-card suits or a casino aesthetic
- Generic neon cyberpunk, Matrix, or “hacker” styling
- Cryptocurrency exchange imagery or language
- A literal phone inside a terminal window as the primary logo
- Generic `</>` marks
- A generic hexagon containing a `D`
- Metallic 3D, chrome, or gaming-clan logo treatments

Android should be used descriptively, for example: “DexDeck is a terminal control plane for Android development.”

DexDeck should not launch with a mascot. The Deckmark is the primary recognizable symbol. A future abstract mascot would require a separate design decision and MUST remain independent of the Android robot and franchise characters.

### 3.18 Lazuli naming usage

`Lazuli` is reserved for:

- The default DexDeck color theme
- Design-system documentation
- Optional release codenames, such as `DexDeck 0.1 “Lazuli”`

It MUST NOT replace the product or executable name. The canonical command remains `dexdeck`.

---

## 4. Core principles

### 4.1 Performance

DexDeck MUST be event-driven and effectively idle when nothing changes.

It MUST NOT run a continuous 30 or 60 FPS render loop. It should render when:

- Application state changes
- Terminal size changes
- User input occurs
- A short animation is active
- A running job emits progress or output

The UI MUST NOT perform blocking filesystem access, process waits, Gradle parsing, Logcat parsing, or network activity on the render/input thread.

Performance is a design requirement rather than a universal contractual latency guarantee. The project SHOULD maintain repeatable benchmarks for:

- Cold process startup
- Warm cached startup
- Input-to-render latency
- Idle CPU usage
- Idle memory usage
- Logcat throughput
- Log filtering latency
- Large-project model loading
- Large task list interaction
- Cancellation latency

Performance regressions SHOULD be investigated before release.

### 4.2 Privacy

DexDeck MUST have zero telemetry permanently.

It MUST NOT include:

- Analytics
- Crash reporting services
- Installation identifiers
- Usage counters
- Remote feature flags
- Automatic update checks
- Background network access
- Diagnostic uploads
- Cloud-backed functionality
- Hidden outbound requests

DexDeck SHOULD not need an HTTP client dependency. If a dependency introduces networking capability transitively, it must not be used for outbound traffic and should be reviewed.

DexDeck itself MUST make no direct network requests. Package managers, Gradle, SDK tools, and user-defined child commands may use the network independently. DexDeck must clearly distinguish its own behavior from child-process behavior.

DexDeck MUST NOT persist raw Logcat or build output unless the user explicitly records or exports it.

DexDeck MUST NOT automatically upload, transmit, or share anything.

### 4.3 Predictability over heuristics

DexDeck MUST prefer explicit actions over speculative “smart” behavior.

Examples:

- `Run` means build, install, and launch.
- `Launch` means launch the already installed application without building.
- `Rerun` means stop and launch the existing installation.
- `Reinstall` means build and replace-install while preserving app data.
- `Clean reinstall` means uninstall, install, and launch and is destructive.

DexDeck MUST NOT guess whether file changes require a rebuild.

### 4.4 Read-only by default

Launching DexDeck in a project MUST NOT create, modify, or delete project files.

The normal launch may write only to operating-system-specific user configuration and cache locations.

Project files may be created only by explicit commands such as:

```bash
dexdeck init
```

DexDeck MUST NOT automatically edit:

- `build.gradle`
- `build.gradle.kts`
- `settings.gradle`
- `settings.gradle.kts`
- `gradle.properties`
- Version catalogs
- Manifests
- Source files
- `.gitignore`

The Gradle integration must be injected externally through an init script or equivalent mechanism.

---

## 5. Non-goals for version 0.1

The following are explicitly outside the version 0.1 scope:

- Integrated source-code editor
- Android layout preview
- Compose preview
- Full Android profiler replacement
- Debugger UI
- Full Debug Adapter Protocol integration
- APK decompiler
- Dependency upgrade automation
- AVD creation wizard
- Wireless debugging pairing wizard
- Multi-device interactive sessions
- Automatic rebuild-on-save
- Play Store publishing
- Firebase console integration
- Public plugin API
- AI assistant or chat UI
- Persistent background daemon
- Editor-specific extensions
- Container-to-host ADB forwarding
- WSL-to-Windows-emulator bridging as a first-class workflow
- Remote cloud services
- Silent SDK installation
- Automatic `.env` loading
- Automatic Gradle build-file modification

These exclusions are deliberate and should be documented publicly.

---

## 6. User experience goals

### 6.1 Primary user

The primary user is an experienced Android developer or small team that:

- Uses Zed, VS Code, Neovim, Vim, Helix, or another non-Android-Studio editor
- Wants a single terminal control surface for Android operations
- Values low idle CPU and memory usage
- Wants excellent Logcat ergonomics
- Works with flavors, build types, multiple modules, tests, and emulators
- Prefers keyboard-driven tools but may also use a mouse
- Expects reliable scriptable CLI behavior

### 6.2 Secondary users

- Android developers who still use Android Studio but want a lighter terminal companion
- CI and automation authors who need machine-readable Android project information
- Remote SSH and tmux users
- Open-source contributors building Android projects without a full IDE

### 6.3 First-launch experience

On first launch:

1. The TUI appears immediately.
2. DexDeck performs a fast filesystem-level project detection.
3. The UI reports the project root, wrapper, and basic environment status.
4. Detailed Gradle model discovery starts automatically.
5. Discovery progress is visible and cancellable.
6. The model is cached after success.
7. A failure produces an actionable explanation and offers degraded mode where possible.

The UI must never present a blank frozen terminal while Gradle configures a project.

Example:

```text
DexDeck

✓ Android Gradle project detected
✓ Gradle wrapper found
● Discovering modules and variants…
  Configuring :build-logic
  Configuring :app
  Configuring :feature:checkout
```

### 6.4 Subsequent launch experience

On subsequent launch:

1. Load the cached model immediately.
2. Restore local session selections where valid.
3. Render the usable interface.
4. Validate the project fingerprint asynchronously.
5. If changed, mark the model stale and refresh.
6. Continue using the previous snapshot until a new one succeeds.

Model status must be explicit:

- Current
- Refreshing
- Stale: build files changed
- Refresh failed: using previous snapshot
- Degraded mode

---

## 7. Technology stack

### 7.1 Primary implementation

- Language: Rust
- TUI framework: Ratatui
- Terminal backend: Crossterm
- Async runtime: Tokio
- CLI parsing: a mature Rust CLI parser such as Clap
- Serialization: Serde
- TOML parsing: a comment-preserving TOML library for files DexDeck may rewrite
- File watching: a cross-platform event-driven watcher
- Logging filters: Rust regex or equivalent bounded deterministic parser

Dependencies should be selected conservatively. Every dependency adds compile time, attack surface, and maintenance risk.

### 7.2 Gradle model bridge

The project contains a small Java 17 Gradle bridge.

Reasons for Java 17:

- AGP 8 projects already require a compatible Java runtime.
- Java avoids adding a Kotlin runtime solely for the bridge.
- The bridge remains small and explicit.
- Java is straightforward to package and load from an init script.

The bridge is embedded in the native executable and extracted to a versioned operating-system cache path when needed.

The installed user-facing artifact remains one native executable.

### 7.3 No persistent daemon

Version 0.1 MUST NOT install or run a DexDeck background daemon.

The foreground process owns:

- File watchers
- Gradle jobs
- ADB tracking
- Logcat streams
- Emulator actions
- Session state

A daemon may be reconsidered only when a concrete, validated use case justifies its lifecycle and protocol complexity.

---

## 8. Repository structure

The repository should begin as a private Cargo workspace with a limited number of crates. Avoid both a monolithic crate and premature fragmentation into many publishable crates.

Recommended structure:

```text
dexdeck/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── LICENSE
├── NOTICE
├── README.md
├── SPEC.md
├── CONTRIBUTING.md
├── SECURITY.md
├── docs/
│   ├── architecture/
│   ├── protocols/
│   └── decisions/
├── crates/
│   ├── dexdeck/              # Binary, CLI entry point, composition root
│   ├── dexdeck-core/         # App state, actions, effects, jobs, errors
│   ├── dexdeck-tui/          # Ratatui rendering and terminal input
│   ├── dexdeck-android/      # ADB, devices, emulator, launch/install
│   ├── dexdeck-gradle/       # Wrapper invocation, bridge integration, model
│   ├── dexdeck-config/       # Config, paths, migrations, project identity
│   ├── dexdeck-protocol/     # Internal/external schemas and versioning
│   └── dexdeck-test-support/ # Fixtures and integration helpers
├── gradle-bridge/
│   ├── settings.gradle.kts
│   ├── build.gradle.kts
│   └── src/main/java/
├── fixtures/
│   ├── basic-app/
│   ├── flavored-app/
│   ├── multi-dimension-app/
│   ├── multi-module/
│   ├── multi-app/
│   ├── composite-build/
│   ├── disabled-variants/
│   └── unsupported-old-agp/
└── .github/
    ├── workflows/
    ├── ISSUE_TEMPLATE/
    └── pull_request_template.md
```

Crates are initially workspace-private. Publishing internal crates is not a version 0.1 goal.

Brand-derived identifiers MUST remain centralized even though the final public name is now selected. Centralization prevents drift across the CLI, TUI, paths, package metadata, documentation, tests, and release automation.

Canonical values:

```text
Product:          DexDeck
Executable:       dexdeck
Environment:      DEXDECK_*
Project config:   .dexdeck/
Cache namespace:  dexdeck
Repository:       drilonrecica/dexdeck
Homebrew formula: dexdeck
Crate package:    dexdeck
Default theme:    lazuli
```

New implementation work MUST NOT introduce the former `DroidDeck`, `droiddeck`, or `DROIDDECK_*` identifiers. A migration path from an unreleased codename is not required unless public artifacts using the old identifiers are deliberately distributed.

---

## 9. Internal application architecture

### 9.1 Unidirectional event model

DexDeck uses an Elm/Redux-like unidirectional architecture:

```text
Input/Event
    ↓
Action
    ↓
State transition/reducer
    ↓
Effect request
    ↓
Async worker
    ↓
Result event
    ↓
State transition
```

Rules:

- Reducers MUST be deterministic and perform no I/O.
- Rendering MUST be pure relative to current state and terminal dimensions.
- Async workers MUST NOT mutate UI state directly.
- Workers emit result events through bounded channels.
- Every long-running effect MUST have cancellation support.
- Every process MUST belong to a supervised job.
- Streams with unbounded potential MUST use bounded queues or ring buffers.

### 9.2 Suggested top-level state

```rust
AppState {
    project: ProjectState,
    gradle: GradleState,
    devices: DeviceState,
    emulators: EmulatorState,
    run: RunState,
    tests: TestState,
    logs: LogState,
    jobs: JobState,
    diagnostics: DiagnosticState,
    config: ConfigState,
    session: SessionState,
    ui: UiState,
}
```

This is conceptual, not a mandated exact Rust layout.

### 9.3 Effect categories

Examples:

- DiscoverProject
- LoadProjectCache
- RefreshProjectModel
- WatchProjectFiles
- StartGradleJob
- CancelJob
- TrackDevices
- StartLogcat
- StopLogcat
- StartEmulator
- StopEmulator
- InstallArtifact
- LaunchApplication
- StopApplication
- RunTests
- OpenSourceLocation
- WriteSharedConfig
- WriteLocalConfig
- ExportLogs

### 9.4 Job model

All long operations use a common job abstraction.

Suggested states:

- Queued
- Starting
- Running
- Cancelling
- Succeeded
- Failed
- Cancelled

Suggested job metadata:

- Job ID
- Kind
- Project identity
- Module
- Variant
- Device
- Command summary
- Start time
- End time
- Duration
- Exit code
- Progress, when available
- Bounded output buffer
- Structured diagnostics

Only lightweight metadata for the most recent 50 jobs is persisted by default. Full output is session-only unless explicitly exported.

### 9.5 Concurrency model

- Only one mutating Gradle operation may run per primary Gradle root by default.
- Logcat, device tracking, emulator boot monitoring, and other non-Gradle operations may run concurrently.
- Gradle itself may use its own internal parallelism.
- The user may queue Gradle actions.
- A future advanced override may permit parallel Gradle invocations, but it is not a version 0.1 requirement.

---

## 10. Project discovery

### 10.1 Root resolution

When launched without `--project`, DexDeck starts from the current working directory and walks upward until it finds a plausible Gradle root.

Signals include:

- `gradlew` or `gradlew.bat`
- `settings.gradle`
- `settings.gradle.kts`
- Root `build.gradle`
- Root `build.gradle.kts`
- Android manifests or conventional Android source trees

The user may override the root:

```bash
dexdeck --project /path/to/project
```

### 10.2 Fast filesystem detection

The first stage must be fast and must not invoke Gradle.

It determines:

- Whether the directory is a Gradle project
- Whether it is probably an Android project
- Primary root location
- Wrapper availability
- Candidate settings file
- Basic SDK/JDK signals

This stage may produce a provisional result but must not claim complete variant knowledge.

### 10.3 Gradle-backed discovery

Detailed modeling must be obtained from Gradle/AGP rather than regex-parsing build scripts.

DexDeck must not attempt to infer the complete build model solely by parsing `build.gradle` or `build.gradle.kts`. Build logic may be dynamic and distributed across:

- Convention plugins
- `buildSrc`
- Included builds
- Custom plugins
- Version catalogs
- Shared Gradle scripts
- Environment-dependent logic

### 10.4 Discovered model

The model should include, when available:

- Gradle version
- AGP version
- Java runtime information
- Kotlin plugin version, when identifiable
- Root and included build identifiers
- Android application modules
- Android library modules
- Build types
- Flavor dimensions in order
- Product flavors and their dimensions
- Enabled variants
- Disabled variants, when discoverable
- Application IDs per variant
- Namespace
- Launcher activity or launchable component when available
- Debuggable state
- Test components
- Unit-test tasks
- Instrumentation-test tasks
- Assemble tasks
- Bundle tasks
- Install tasks
- Lint/verification tasks
- Artifact locations where known
- Compile SDK
- Target SDK
- Minimum SDK
- Resolved ADB path or SDK components when safely available

### 10.5 Included builds

Included builds and composite builds must be considered for model correctness.

Version 0.1 exposes only primary-project Android modules as first-class navigation items. Tasks from included builds may be available through the task palette or advanced task browser.

### 10.6 Degraded mode

If the bridge cannot produce a full model:

- Explain the failure precisely.
- Preserve any usable cached model.
- Offer degraded task mode.
- Permit ADB and Logcat features where possible.
- Permit arbitrary Gradle task execution.
- Permit manually configured profiles.
- Never claim full support when operating on heuristics.

Potential fallback discovery may use wrapper-level task listing, but output must be labeled as degraded and may be slower.

---

## 11. Gradle bridge

### 11.1 Packaging

The bridge is a Java 17 module built as part of the repository release process.

Its artifacts are embedded in the native executable and extracted to a versioned cache directory such as:

```text
Linux:
~/.cache/dexdeck/bridge/<bridge-hash>/

macOS:
~/Library/Caches/dexdeck/bridge/<bridge-hash>/

Windows:
%LOCALAPPDATA%\dexdeck\cache\bridge\<bridge-hash>\
```

Extraction must be atomic and verified by content hash.

### 11.2 Project isolation

The bridge MUST NOT be added to the project’s build files.

It should be introduced through a Gradle init script or init plugin and invoked through the project’s own wrapper.

The bridge must not permanently mutate the Gradle project.

### 11.3 Wrapper usage

Always prefer the project wrapper:

- Unix: `./gradlew`
- Windows: `gradlew.bat`

A system Gradle installation may be used only when the wrapper is absent and the user explicitly approves or configures that behavior.

### 11.4 Daemon behavior

Use Gradle’s normal daemon by default.

DexDeck must not stop the shared Gradle daemon when exiting or cancelling a job.

A troubleshooting option may add `--no-daemon` or expose an explicit `gradle --stop` action, but neither is the default.

### 11.5 Bridge output protocol

The bridge protocol is versioned JSON Lines.

For robustness, structured output should be written to an explicit temporary output file rather than mixed with arbitrary Gradle stdout. Gradle stdout/stderr can still be streamed separately for progress and diagnostics.

Example invocation concept:

```text
gradlew \
  --init-script <bridge-init-script> \
  dexdeckModel \
  -Ddexdeck.output=<temp-jsonl-path> \
  --console=plain
```

Exact task and property names may change internally, but the behavior is normative.

Example records:

```json
{"protocolVersion":1,"type":"build","root":"/project","gradleVersion":"8.11","agpVersion":"8.7.0"}
{"protocolVersion":1,"type":"module","path":":app","kind":"application"}
{"protocolVersion":1,"type":"dimension","module":":app","name":"environment","order":0}
{"protocolVersion":1,"type":"flavor","module":":app","name":"demo","dimension":"environment"}
{"protocolVersion":1,"type":"buildType","module":":app","name":"debug","debuggable":true}
{"protocolVersion":1,"type":"variant","module":":app","name":"demoDebug","enabled":true,"applicationId":"com.example.demo"}
{"protocolVersion":1,"type":"task","module":":app","name":"assembleDemoDebug","kind":"assemble","variant":"demoDebug"}
{"protocolVersion":1,"type":"complete","durationMs":1462}
```

Every record must contain a protocol version.

### 11.6 Adapter strategy

AGP-specific interaction belongs behind adapters selected by detected AGP version.

Adapters should expose a stable internal model.

The bridge should fail with structured diagnostics when:

- AGP is unsupported
- Required APIs are unavailable
- A plugin throws during configuration
- The output path is not writable
- The bridge protocol is incompatible
- The project uses an unsupported Gradle version

### 11.7 Configuration cache

DexDeck must not automatically enable configuration cache or modify `gradle.properties`.

It may:

- Detect whether configuration cache is enabled
- Report compatibility problems
- Allow project/user configuration to add appropriate Gradle flags
- Provide a local performance advisor in a later milestone

Performance advice must be deterministic and local.

---

## 12. Cache and local state

### 12.1 Storage locations

Shared project configuration:

```text
<project>/.dexdeck/config.toml
```

This file is optional and created only through explicit initialization or user action.

Per-user project configuration:

```text
Linux:
~/.config/dexdeck/projects/<project-hash>/config.toml

macOS:
~/Library/Application Support/dexdeck/projects/<project-hash>/config.toml

Windows:
%APPDATA%\dexdeck\projects\<project-hash>\config.toml
```

Generated cache:

```text
Linux:
~/.cache/dexdeck/projects/<project-hash>/

macOS:
~/Library/Caches/dexdeck/projects/<project-hash>/

Windows:
%LOCALAPPDATA%\dexdeck\cache\projects\<project-hash>\
```

### 12.2 Project identity

The cache directory name is a stable hash derived from:

- Canonical primary project path
- Cache namespace version

The canonical path should not appear in the directory name.

The cache may store the path internally because it is local-only.

Repository trust identity should additionally consider the Git remote when available.

### 12.3 Recommended cache files

```text
model.json          # Last successful normalized project model
fingerprint.json    # Inputs and hashes used for invalidation
session.json        # Local selections and UI state
jobs.json           # Last 50 lightweight job records
filters.json        # Saved local Logcat filters
trust.json          # Project command trust decision
```

Files must include schema versions.

Writes must be atomic: write a temporary file, flush, then replace.

Corrupt cache files must be ignored safely with a local warning. Cache corruption must never prevent opening the project.

### 12.4 Fingerprinting

The project fingerprint should cover build-model-relevant files, including:

- `settings.gradle`
- `settings.gradle.kts`
- Root and module `build.gradle`
- Root and module `build.gradle.kts`
- `gradle.properties`
- `gradle/libs.versions.toml`
- `gradle/wrapper/gradle-wrapper.properties`
- Relevant files under `buildSrc`
- Included-build settings and convention-plugin sources
- Applied shared Gradle script files where discoverable
- DexDeck shared configuration

Fingerprinting must avoid scanning arbitrary source trees.

A two-stage strategy is recommended:

1. Fast metadata comparison using known file list, size, and modification time.
2. Content hashing only for changed or newly discovered inputs.

File watcher events should be debounced. The model should be marked stale immediately, but a refresh should wait until active Gradle work finishes unless the user explicitly forces it.

### 12.5 Session restoration

DexDeck should remember locally:

- Last selected application module
- Last selected variant
- Last active device identity
- Last Logcat filter
- Last workspace/panel
- Last run profile
- Last successful action

If the stored device is unavailable, DexDeck must select no device rather than silently choosing a different device.

---

## 13. Configuration

### 13.1 Configuration is optional

DexDeck must be useful without any project configuration.

Configuration is required only for features such as:

- Shared run profiles
- Custom commands
- Editor command selection
- Keybinding customization
- Project-specific Gradle flags
- Logcat preferences
- Project defaults

### 13.2 Precedence

Recommended precedence from highest to lowest:

1. Explicit CLI arguments
2. Per-user project configuration
3. Shared project configuration
4. Detected project values
5. Built-in defaults

Environment variables are inherited by child processes. Secret values should be referenced by name rather than stored.

### 13.3 Schema versioning

Every TOML configuration must include a schema version.

Example:

```toml
schema_version = 1
```

Unknown fields should produce warnings, not silent deletion.

Invalid values must produce precise diagnostics with file path and line/column where possible.

Local-only configuration may be migrated automatically with backup and atomic replacement.

Shared configuration migrations require explicit confirmation and must preserve comments and formatting.

### 13.4 Example shared configuration

```toml
schema_version = 1

[project]
default_module = ":app"
default_variant = "demoDebug"

[gradle]
arguments = ["--stacktrace"]

[ui]
keymap = "default"
reduced_motion = false
unicode = "auto"

[logcat]
buffer_mib = 32
minimum_priority = "debug"
default_scope = "application"

[editor]
command = ["zed", "{path}:{line}:{column}"]

[profiles.demo]
module = ":app"
variant = "demoDebug"
device = "last-used"
launch_mode = "launcher"

[profiles.checkout-deeplink]
module = ":app"
variant = "demoDebug"
deep_link = "example://checkout"
intent_extras = { source = "dexdeck" }

[profiles.local-backend.environment]
API_BASE_URL = "http://localhost:8080"
API_TOKEN = { from_env = "DEMO_API_TOKEN" }

[commands.mock-server]
command = ["docker", "compose", "up", "mock-api"]
working_directory = "."
```

### 13.5 Argument safety

Commands and Gradle arguments must be represented as arrays, never a single shell string.

DexDeck should execute programs directly using argv semantics.

The following is valid:

```toml
command = ["docker", "compose", "up", "mock-api"]
```

The following must not be accepted as an implicit shell command:

```toml
command = "docker compose up mock-api | tee output.log"
```

A future explicit shell mode may exist, but it must be visibly opt-in and subject to project trust.

### 13.6 Secrets

DexDeck must not store secret values in shared configuration.

Secret references may use environment variables. DexDeck must never display the resolved value in:

- UI
- Debug logs
- Job metadata
- Error messages
- Exported diagnostics

DexDeck must not automatically read `.env` files.

---

## 14. Repository trust and custom commands

Gradle project execution already runs repository-owned code. DexDeck custom commands introduce an additional execution path and therefore require explicit trust.

Before executing project-defined custom commands, DexDeck asks the user to:

- Trust once
- Trust this project
- Cancel

Stored trust identity should hash:

- Canonical project path
- Git remote URL, when available
- Trust schema version

Trust is invalidated when the Git remote changes.

If no remote exists, use path plus repository metadata where available.

DexDeck must show the command argv and working directory before first execution.

---

## 15. TUI design

### 15.1 Overall model

The TUI is a persistent operational dashboard, not a chat transcript.

The interface should always make important state visible:

- Project
- Application module
- Variant
- Active device
- Running app state
- Active Gradle job
- Logcat scope and filters
- Model freshness

### 15.2 Default layout

Full layout at approximately 100×30 and larger:

```text
┌ DexDeck ─ project ─ module ─ variant ─ device ─ model status ─────────────┐
│ [Run] [Build] [Test] [Logs] [Devices] [Tasks]                 Ctrl+P Commands│
├──────────────────────┬───────────────────────────────────────────────────────┤
│ Navigation           │ Active workspace                                      │
│ Modules              │ Logcat / Tests / Jobs / Devices / Tasks / Doctor      │
│ Variants             │                                                       │
│ Profiles             │                                                       │
├──────────────────────┼───────────────────────────────────────────────────────┤
│ Device summary       │ Active jobs                                            │
├──────────────────────┴───────────────────────────────────────────────────────┤
│ Context shortcuts • status • warnings • queue                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

The design should be dense in capability but progressively disclosed visually.

### 15.3 Responsive behavior

- Full layout: roughly `100×30` and above
- Compact layout: around `80×24`
- Single-workspace mode below compact dimensions
- Below approximately `40×10`, show a resize message rather than corrupt rendering

The exact thresholds may be tuned through testing.

### 15.4 Workspaces

Version 0.1 should include:

- Project/overview
- Run profiles
- Logcat
- Tests
- Devices and existing emulators
- Gradle tasks
- Jobs/history
- Doctor/environment

### 15.5 Keyboard interaction

Neutral default bindings:

```text
Arrow keys / Tab     Navigate
Enter                Select / open / confirm
Esc                  Close overlay / return
Ctrl+P               Command palette
Ctrl+R               Run selected profile or current target
Ctrl+B               Build
Ctrl+T               Test
Ctrl+L               Focus Logcat
?                    Contextual help
/                    Search in active workspace where applicable
```

An optional Vim preset adds:

- `h`, `j`, `k`, `l`
- `g`, `G`
- `/`
- Other conventional navigation bindings

Bindings map to stable named actions, not arbitrary UI coordinates.

### 15.6 Mouse interaction

Primary actions and navigation must be mouse-accessible:

- Click tabs and panels
- Select list rows
- Scroll
- Resize panes where supported
- Activate buttons
- Open context menus

Keyboard operation remains the primary design target because mouse behavior varies across terminals, SSH, and multiplexers.

### 15.7 Command palette

The command palette is a central interface.

It should support:

- Fuzzy action search
- Module selection
- Variant selection
- Device selection
- Task execution
- Profile execution
- Emulator actions
- Help and settings navigation

Each palette item should show its shortcut and context requirements.

### 15.8 Animation

Animations must be restrained and short:

- Spinners
- Progress glyphs
- Brief highlight interpolation
- Success/error pulse
- Small pane transition cues

Rules:

- Maximum active animation rate should be approximately 30 FPS.
- Normal rendering is event-driven.
- Idle animations stop.
- Reduced-motion mode disables nonessential animation.
- Animations must not interfere with terminal readability or SSH use.

### 15.9 Unicode and color

- Unicode is enabled automatically when supported.
- ASCII fallback must remain fully functional.
- Color must never be the only status signal.
- Support true color when available, then 256-color, then conservative 16-color fallback.
- The UI should work on light and dark terminal backgrounds.

### 15.10 Visual direction

The core workspace should be restrained, technical, professional, and recognizably DexDeck without sacrificing information density.

Use:

- Lazuli semantic theme tokens
- Sparse Deckmark usage in empty states, documentation, and the application identity area
- Angular layered geometry where decoration is appropriate
- Clear focus, job, diagnostic, and model-freshness states
- Authentic terminal screenshots for public materials

Avoid:

- Fake hacker aesthetics
- Permanent giant ASCII logos
- Excessive animation
- Excessive Android-green coloring
- Direct imitation of Android Studio or Google visual systems
- Chat-centric UI patterns
- Dragon Ball or Red Ribbon imagery
- Robot-head or antenna motifs
- Literal playing cards, casino imagery, or card suits
- Crypto/fintech visual language
- Mandatory Nerd Font glyphs

The Deckmark, not a mascot, is the primary symbol. Brand expression must remain secondary to operational clarity.

---

## 16. CLI design

### 16.1 Single binary

The same executable provides TUI and noninteractive CLI behavior.

Running without a subcommand opens the TUI:

```bash
dexdeck
```

### 16.2 Core commands

Proposed version 0.1 commands:

```bash
dexdeck
dexdeck init
dexdeck doctor

dexdeck project inspect
dexdeck modules list
dexdeck variants list
dexdeck devices list
dexdeck emulators list

dexdeck build
dexdeck install
dexdeck launch
dexdeck run
dexdeck rerun
dexdeck reinstall
dexdeck stop

dexdeck test
dexdeck logs
dexdeck gradle <task> [<task> ...]

dexdeck emulator start <name>
dexdeck emulator cold-boot <name>
dexdeck emulator wipe <name>
dexdeck emulator stop <name>
```

Names may evolve before 1.0, but changes must be documented.

### 16.3 Global options

Recommended options:

```text
--project <path>
--module <gradle-path>
--variant <name>
--device <serial-or-selector>
--profile <name>
--format <human|json|jsonl>
--gradle-arg <arg>         # repeatable
--no-color
--ascii
--debug-log <path>
--config <path>
--yes                     # only for explicitly documented confirmations
```

### 16.4 Machine-readable output

Structured output is a public versioned API from the beginning.

- Snapshot-style commands use JSON.
- Streaming commands use JSONL.
- Every record includes a schema version.
- Human-readable output may evolve more freely.
- Machine fields must not change silently.

Example:

```json
{
  "schemaVersion": 1,
  "project": {
    "root": "/project",
    "modules": []
  }
}
```

Example JSONL:

```json
{"schemaVersion":1,"type":"jobStarted","jobId":"...","kind":"build"}
{"schemaVersion":1,"type":"diagnostic","severity":"error","message":"..."}
{"schemaVersion":1,"type":"jobFinished","jobId":"...","exitCode":1}
```

### 16.5 Exit codes

Recommended categories:

```text
0   Success
1   Operation failed
2   Invalid CLI usage or configuration
3   Project not found or unsupported
4   Environment/tooling missing
5   Device/emulator error
6   Cancelled by user
7   Protocol or cache incompatibility
8   Internal DexDeck failure
```

Exact numbers may change before the first public compatibility commitment, but categories should remain clear.

---

## 17. Build, install, and run behavior

### 17.1 Explicit actions

- **Build:** Execute the appropriate assemble/build task.
- **Install:** Install an existing or newly built artifact on the active device.
- **Launch:** Start the configured activity or deep link without building.
- **Run:** Build, install, and launch.
- **Rerun:** Stop and launch the existing installed application.
- **Reinstall:** Build, replace-install, and launch while preserving app data.
- **Clean reinstall:** Build, uninstall, install, and launch; destructive and requires confirmation.
- **Stop:** Force-stop the selected application package.

### 17.2 Default installation policy

Default behavior:

- Replace existing installation: enabled
- Test APK installation: allowed when required
- Version downgrade: disabled
- Grant all runtime permissions: disabled
- Uninstall first: disabled
- Clear app data: disabled

Run and reinstall preserve application data by default.

Separate explicit actions provide:

- Install with downgrade
- Clear application data
- Uninstall
- Clean reinstall

### 17.3 Release variants

Release and non-debuggable variants may be displayed.

Before installing or launching a release/non-debuggable variant, DexDeck should provide a clear confirmation unless the user has configured that profile explicitly.

DexDeck must never read, display, copy, or cache signing secrets.

It relies on the project’s existing Gradle signing configuration.

### 17.4 Run profiles

Profiles may specify:

- Module
- Variant
- Active device selector
- Launcher activity
- Explicit activity
- Deep link
- Intent action
- Intent categories
- Intent extras
- Gradle properties
- Environment references
- Whether to start a configured existing emulator if offline

DexDeck must never silently choose and boot an emulator.

---

## 18. Android SDK and tooling resolution

### 18.1 SDK resolution order

Explicit CLI or user configuration may override detection.

Otherwise use, in priority order:

1. SDK/ADB information resolved from the project or AGP model
2. Project `local.properties` SDK path
3. Relevant Android SDK environment variables
4. Known platform-specific installation locations

DexDeck must report which SDK it selected.

### 18.2 Bundling policy

DexDeck MUST NOT bundle:

- ADB
- Emulator
- SDK Manager
- Build tools
- Platform tools
- Android SDK packages

It uses the user’s Android SDK installation.

### 18.3 SDK installation

DexDeck may diagnose missing SDK components and show or execute an explicitly approved `sdkmanager` command.

It must not silently install packages or accept licenses.

### 18.4 Doctor command

`dexdeck doctor` should inspect:

- Java availability and version
- Gradle wrapper
- Android SDK location
- ADB
- Emulator package
- Platform packages relevant to the project
- Device visibility
- AGP support level
- Terminal capabilities
- Editor command configuration
- Cache and config permissions

Example:

```text
✓ Java 17
✓ Gradle wrapper
✓ Android SDK
✓ adb
✗ Emulator package
✗ Platform android-36

Suggested:
  sdkmanager "emulator" "platforms;android-36"
```

---

## 19. Device and emulator management

### 19.1 ADB server

- Start ADB lazily when an ADB-dependent action is requested.
- Use the selected SDK’s ADB executable.
- Never stop the ADB server when DexDeck exits.
- Provide an explicit ADB restart action for troubleshooting.

### 19.2 Device tracking

Use a long-lived device-tracking mechanism such as `adb track-devices` rather than aggressive polling.

Track:

- Serial
- State
- Model
- Product/device identifiers
- API level where obtainable
- Transport type
- Emulator/physical classification
- Authorization/offline status

### 19.3 Wireless devices

Devices already visible to ADB are supported regardless of USB or TCP transport.

A full pairing wizard is deferred.

### 19.4 Existing emulator management

Version 0.1 supports existing AVDs:

- List
- Start
- Cold boot
- Wipe data
- Stop
- Show key properties
- Monitor boot completion

DexDeck does not create new AVDs in version 0.1.

DexDeck never automatically stops an emulator when the TUI exits, including emulators it launched.

### 19.5 Device API support

Official target floor: Android 6.0 / API 23 and newer.

Basic ADB behavior on older devices may work on a best-effort basis.

This device floor is separate from the project’s `minSdk`.

---

## 20. Logcat

### 20.1 Importance

Structured Logcat is a flagship feature and a major reason to keep DexDeck open during development.

A plain colored wrapper around `adb logcat` is insufficient.

### 20.2 Default scope

Default Logcat scope is the selected application ID and all of its active processes.

Example processes:

```text
com.example.app
com.example.app:sync
com.example.app:remote
```

The user may narrow to one process or switch to full-device logs.

DexDeck must account for process restarts and newly created secondary processes.

### 20.3 Stream design

Recommended pipeline:

```text
ADB stdout
   ↓
Bounded byte reader
   ↓
Incremental parser
   ↓
Bounded event channel
   ↓
Byte-bounded ring buffer
   ↓
Compiled filters
   ↓
Viewport renderer
```

A noisy device must not block keyboard input or UI rendering.

### 20.4 Parsed fields

Where available, parse:

- Timestamp
- Process ID
- Thread ID
- UID
- Priority
- Tag
- Message
- Package/process mapping
- Stack-trace continuation
- Crash boundaries
- Fatal exception metadata
- Native crash markers
- ANR-related markers, best effort

### 20.5 Memory policy

Log storage is byte-bounded.

- Minimum configurable buffer: 8 MiB
- Default: 32 MiB
- Common values: 64, 128, 256 MiB
- Maximum: 1 GiB
- Warning above: 256 MiB

When full, discard the oldest complete entries and show a dropped-entry indicator.

Use compact data structures and avoid repeated string copies.

### 20.6 Filtering

Support:

- Minimum priority
- Include/exclude tags
- Include/exclude package/process
- Plain text search
- Regex search
- Case sensitivity
- Saved local presets
- Crash-only or error-focused views

Filters should be compiled and cached. The implementation should avoid rescanning the entire buffer on every keystroke where incremental behavior is possible.

### 20.7 Persistence

Raw Logcat is memory-only by default.

Explicit user actions may:

- Export current visible logs
- Export full buffered logs
- Start recording to a user-selected path
- Stop recording

No automatic session recording is permitted.

### 20.8 User actions

Recommended actions:

- Pause/resume display without stopping capture
- Clear in-memory buffer
- Toggle application/full-device scope
- Select process
- Copy line
- Copy grouped stack trace
- Jump between crashes/errors
- Save filter preset
- Export
- Record

---

## 21. Testing

### 21.1 Supported test types in version 0.1

- Local JVM unit tests
- Connected Android instrumentation tests
- Arbitrary project-defined test tasks

Gradle-managed devices are deferred to a later milestone unless implementation proves straightforward without delaying core work.

### 21.2 Granularity

Local unit tests should support:

- Task
- Module
- Class
- Method

Instrumentation tests should support:

- Module
- Package
- Class
- Method where supported reliably by the runner and project

Editor-aware “test current file” behavior is deferred.

### 21.3 Invocation

DexDeck should use project-standard Gradle mechanisms and test filters.

It must display the resolved command and task in job details.

### 21.4 Results

Parse and present:

- JUnit XML
- Instrumentation results
- Passed/failed/skipped counts
- Duration
- Failure message
- Stack trace
- Source location where resolvable

Actions:

- Rerun all
- Rerun failed
- Rerun selected class/method
- Open source location
- Copy failure
- Expand raw output

### 21.5 Diagnostics milestone split

Version 0.1 must include useful compiler errors and test failures.

Advanced lint dashboards, manifest-merger visualization, and rich SARIF browsing may follow immediately after the core release if they risk delaying the daily loop.

---

## 22. Diagnostics and source navigation

### 22.1 Structured diagnostics

DexDeck should normalize diagnostics from:

- Kotlin compiler
- Java compiler
- Android resource compilation
- Manifest merger
- Gradle task failures
- ADB installation failures
- JUnit tests
- Instrumentation tests
- Android Lint where practical

Suggested fields:

```text
severity
category
message
file
line
column
module
variant
task
raw_context
suggested_action
```

### 22.2 Editor integration

Source opening is command-template based.

Example:

```toml
[editor]
command = ["zed", "{path}:{line}:{column}"]
```

Built-in presets may cover:

- Zed
- VS Code
- Neovim
- Vim
- Helix
- Android Studio / IntelliJ as an optional fallback

Respect `$VISUAL` and `$EDITOR` when no explicit command is configured.

Do not use parent-process heuristics in version 0.1 because terminals, multiplexers, and wrappers make them unreliable.

---

## 23. Gradle tasks and custom commands

### 23.1 Gradle task browser

Provide searchable task execution with:

- Task name
- Group
- Description
- Module/build origin
- Variant association where known
- Recent usage

Large task lists must be virtualized or filtered efficiently.

### 23.2 Gradle arguments

Allow additional Gradle arguments at:

- Shared project level
- User project level
- Run-profile level
- One-time CLI level

Arguments are arrays.

DexDeck retains control of internal model-export and machine-output flags.

### 23.3 Custom commands

Custom commands may launch non-Gradle tools such as:

- Backend servers
- Docker Compose
- Code generation
- Database seeding
- Formatters

They are subject to repository trust.

Version 0.1 executes argv-based commands only.

---

## 24. Process supervision and cancellation

### 24.1 Process trees

Every external process must be supervised as a process tree.

Implementation requirements:

- Unix: use process groups or equivalent
- Windows: use Job Objects or equivalent
- Avoid orphaned Gradle, ADB, shell, and test processes

### 24.2 Cancellation behavior

First cancellation:

1. Mark job as cancelling.
2. Send a graceful interrupt to the process group.
3. Wait a bounded grace period.

Second cancellation or grace timeout:

1. Forcefully terminate the complete process tree.
2. Mark partial model/artifact output invalid.

Never terminate the shared Gradle daemon as a side effect of cancelling one build.

### 24.3 TUI exit behavior

On exit:

- Ask how to handle active foreground jobs if needed.
- Do not stop emulators.
- Do not stop ADB server.
- Do not stop Gradle daemon.
- Restore terminal state.
- Persist only permitted local state.

---

## 25. Error handling

### 25.1 Error taxonomy

Errors should be categorized:

- Configuration error
- Project detection error
- Unsupported project/AGP
- Gradle bridge error
- Gradle operation failure
- SDK/tool missing
- Device unavailable
- Emulator failure
- ADB failure
- Test failure
- Cache corruption
- Permission error
- Internal invariant failure

### 25.2 Error quality

Errors must answer:

- What failed?
- What was DexDeck trying to do?
- What project/module/variant/device was involved?
- Is the previous model still usable?
- What can the user do next?
- Where is the raw output?

Avoid generic “Something went wrong” messages.

### 25.3 Terminal restoration

Terminal lifecycle must be managed by a centralized RAII guard.

On panic or unexpected termination:

- Disable raw mode
- Leave alternate screen
- Disable mouse capture
- Restore cursor
- Flush terminal output
- Print a concise error to normal stderr

DexDeck must not leave the terminal corrupted.

### 25.4 Crash behavior

No crash report is uploaded.

Default crash output:

```text
DexDeck terminated unexpectedly.

Terminal state was restored.
Run again with:
  dexdeck --debug-log ./dexdeck-debug.log
```

Crash dumps are opt-in only.

---

## 26. Debug logging and diagnostics

DexDeck does not write persistent application logs by default.

Explicit options:

```bash
dexdeck --debug-log ./dexdeck-debug.log
```

or an environment-controlled verbosity level.

Without a file destination, debugging information may appear in an internal diagnostics panel and disappears on exit.

Debug logs must redact:

- Secret environment values
- Signing credentials
- Password-like Gradle properties
- Access tokens

A future diagnostic bundle must be local, user-reviewed, and redacted before the user chooses to share it.

---

## 27. Remote terminal behavior

DexDeck must work in:

- Local terminals
- SSH sessions
- tmux
- screen-like multiplexers where terminal capabilities permit
- Editor-integrated terminals

It must detect terminal capabilities rather than require a graphical desktop.

Version 0.1 uses whatever ADB environment is visible to the running process.

Host/container and WSL emulator forwarding are deferred.

---

## 28. Packaging and installation

### 28.1 Release channels

Recommended order:

1. GitHub release archives with checksums
2. POSIX shell installer
3. PowerShell installer
4. Official Homebrew tap with prebuilt bottles
5. Crates.io as a developer fallback
6. WinGet after Windows support is credible
7. Community-maintained Nix, AUR, Scoop, Debian, RPM, and other packages

### 28.2 Homebrew

Use an official project tap initially rather than depending on immediate acceptance into a central repository.

Install experience should resemble:

```bash
brew install <organization>/tap/dexdeck
```

Bottles should be prebuilt. Users should not need Rust installed.

### 28.3 Cargo installation

`cargo install` is a fallback for Rust users, not the primary distribution path.

### 28.4 Release tooling

A release orchestrator such as cargo-dist may be used, but release architecture must not depend on one vendor-specific implementation.

Generated workflows should be committed and reviewable.

### 28.5 Direct update behavior

DexDeck itself performs no update checks.

Updates are handled by:

- Homebrew
- WinGet
- Cargo
- GitHub Releases
- User-selected package manager

---

## 29. Licensing and governance

- License: Apache-2.0
- Include `LICENSE`
- Include `NOTICE`
- Require DCO sign-off for contributions
- Include `CONTRIBUTING.md`
- Include `SECURITY.md`
- Include issue and pull-request templates
- Do not include a Code of Conduct file

Contribution and moderation rules should remain concise and focused on technical collaboration, project relevance, and maintainer authority.

---

## 30. CI and testing strategy

### 30.1 Pull-request CI

Every pull request should run:

- Rust formatting
- Rust linting
- Unit tests
- Config parsing tests
- Protocol compatibility tests
- Cache migration tests
- TUI snapshot tests
- Gradle bridge compilation
- Fixture-project model tests not requiring an emulator
- Cross-platform compilation checks where practical

### 30.2 Scheduled or release CI

Scheduled and release workflows should include:

- Real Android SDK setup
- ADB integration
- Emulator boot
- APK installation
- App launch verification
- Connected instrumentation tests
- macOS smoke tests
- Linux smoke tests
- Windows smoke tests
- Package installation tests
- Release artifact checksum validation

### 30.3 Fixture matrix

Fixtures should cover:

- Basic single-module application
- Kotlin DSL project
- Groovy DSL project
- Multiple application modules
- Application plus libraries
- Multiple flavor dimensions
- Disabled variants
- Convention plugins
- `buildSrc`
- Included build / composite build
- Custom task names
- AGP 8 minimum-supported version
- Current supported AGP version
- Unsupported old AGP
- Missing wrapper
- Broken configuration

### 30.4 Property and fuzz testing

Where valuable, use property/fuzz tests for:

- Logcat parser
- JSONL protocol parser
- ANSI/build-output parser
- Config migrations
- Path normalization
- Ring buffer behavior

---

## 31. Performance engineering guidance

### 31.1 General rules

- Prefer streaming over collecting.
- Prefer bounded queues over unbounded channels.
- Avoid cloning large strings.
- Use compact IDs and intern repeated metadata where beneficial.
- Virtualize large lists.
- Parse off the UI thread.
- Do not wake the render loop without state changes.
- Do not aggressively poll files or devices.
- Measure before complex optimization.

### 31.2 Logcat-specific rules

- Incremental parsing
- Byte-bounded memory
- Batch UI updates when output rate is high
- Compile filters once
- Avoid reformatting off-screen entries
- Maintain indexes for priority/tag/process where justified

### 31.3 Build-output rules

- Stream output to a bounded job buffer
- Extract diagnostics incrementally
- Do not retain unlimited ANSI output
- Preserve full current-job output only while useful

### 31.4 Binary optimization

Runtime performance is more important than artifact size.

Release configuration should be benchmarked rather than selected dogmatically. Thin LTO, codegen units, symbol stripping, allocator choice, and profile-guided optimization may be evaluated.

Do not use `panic = abort` if it compromises terminal restoration and diagnostic quality without a robust outer guard.

Do not introduce runtime downloads merely to reduce binary size.

---

## 32. v0.1 milestone plan

### Milestone 0: Foundation

Deliver:

- Cargo workspace
- CLI skeleton
- Core action/state/effect architecture
- Terminal lifecycle guard
- Job abstraction
- Config and protocol versioning
- Basic CI
- No Android-specific functionality required yet

Definition of done:

- TUI opens and exits cleanly
- Panic restores terminal
- CLI can emit versioned JSON
- State/effect boundaries are tested

### Milestone 1: Project understanding

Deliver:

- Filesystem project detection
- Java Gradle bridge
- AGP 8 adapter
- Module/variant/task model
- Cache and fingerprinting
- File watching
- CLI commands:
  - `project inspect`
  - `modules list`
  - `variants list`
- Fixture coverage
- Degraded mode

Definition of done:

- Flavored and multi-module fixture projects are modeled correctly
- Cache loads without invoking Gradle when current
- Build-file changes mark the model stale
- Bridge failure does not corrupt the previous cache

### Milestone 2: Devices and execution

Deliver:

- SDK resolution
- ADB supervision
- Device tracking
- Existing emulator listing/start/stop/cold boot/wipe
- Build/install/launch/stop/run/reinstall
- Gradle job queue
- Cross-platform process cancellation

Definition of done:

- User can select module, variant, and one device
- User can run an app without Android Studio
- Data is preserved during ordinary run/reinstall
- Cancellation leaves no orphaned child process

### Milestone 3: Logcat

Deliver:

- Streaming parser
- Package/process mapping
- Byte-bounded ring buffer
- Filters
- Crash grouping
- Search
- Saved local presets
- Export and explicit recording
- Logcat TUI workspace

Definition of done:

- High-volume logs do not freeze input
- Default view follows all selected-app processes
- Memory remains bounded
- Nothing is written to disk without explicit recording/export

### Milestone 4: Tests and diagnostics

Deliver:

- Local unit tests
- Connected instrumentation tests
- Class/method filters
- JUnit parsing
- Compiler diagnostics
- Source navigation
- Rerun failed tests

Definition of done:

- User can run and inspect tests from TUI and CLI
- Failures link to source locations where possible
- Machine-readable results are versioned

### Milestone 5: v0.1 hardening and release

Deliver:

- Responsive layouts
- Mouse support
- Keybinding customization
- Vim preset
- Doctor command
- Privacy audit
- Performance benchmarks
- Packaging
- Documentation
- macOS/Linux stable release
- Windows experimental release

Definition of done:

- No direct network behavior
- No telemetry
- Normal launch does not modify project files
- Installation works through release archive and Homebrew tap
- Terminal restoration is reliable across supported platforms

---

## 33. Version 0.1 acceptance criteria

Version 0.1 is acceptable when all of the following are true.

### Project modeling

- Detects a native Android Gradle project from a nested directory.
- Uses the project wrapper.
- Correctly lists multiple application modules.
- Correctly lists flavor dimensions, flavors, build types, and enabled variants.
- Loads a valid cache immediately on subsequent startup.
- Refreshes after relevant build-file changes.
- Enters degraded mode with a clear reason when full modeling fails.

### Daily run loop

- Select module, variant, and device.
- Build selected variant.
- Install it.
- Launch it.
- Stop it.
- Rerun without rebuilding.
- Reinstall without clearing data.
- Perform an explicit destructive clean reinstall with confirmation.

### Logcat

- Default application-scoped view includes secondary processes.
- Parsing and rendering remain responsive under high output.
- Buffer defaults to 32 MiB and is configurable up to 1 GiB.
- Old entries are discarded predictably.
- User can search, filter, copy, export, and explicitly record.
- No automatic disk persistence.

### Tests

- Runs local unit tests.
- Runs connected instrumentation tests.
- Supports narrower test selection where available.
- Parses results and displays failures.
- Supports rerunning failed tests.

### CLI/TUI parity

- Core run, test, device, model, task, and log operations are available through both surfaces where meaningful.
- JSON and JSONL schemas are versioned.
- Human and machine output are separated.

### Privacy

- No analytics or telemetry code.
- No outbound update checks.
- No direct HTTP requests.
- No automatic uploads.
- No persistent raw logs by default.
- No secret values printed.
- Project custom commands require trust.

### Reliability

- Cancelling a job does not leave orphaned process trees.
- Exiting does not stop ADB, emulators, or Gradle daemon.
- Panics restore terminal state.
- Cache corruption does not block startup.
- Normal launch leaves Git status unchanged.

---

## 34. Deferred roadmap candidates

Potential post-v0.1 work, prioritized only after real user feedback:

- Rich Android Lint and SARIF dashboard
- Manifest merger explorer
- Gradle performance advisor
- Debugger attachment helpers and DAP integration
- Gradle-managed devices
- Multi-device batch test/install
- AVD creation wizard
- Wireless pairing wizard
- Desktop notifications, opt-in
- Editor extensions built on the stable CLI protocol
- Container/WSL device forwarding
- Public plugin API after internal abstractions stabilize
- Dependency inspection and upgrade assistance

None of these should delay a reliable core daily loop.

---

## 35. Rules for AI coding agents

AI coding agents working on this repository must follow these rules:

1. Treat this specification as authoritative unless a newer ADR or maintainer instruction explicitly supersedes it.
2. Do not reuse architecture from the previous repository implementation.
3. Do not add telemetry, analytics, crash reporting, remote flags, or update checks.
4. Do not add a direct network feature without an explicit specification revision.
5. Do not modify project files during normal launch.
6. Do not parse Gradle build scripts as the primary source of variant truth.
7. Do not add a background daemon in version 0.1.
8. Do not implement a chat-centric TUI.
9. Do not use unbounded queues for logs or process output.
10. Do not silently swallow model, cache, or process errors.
11. Do not store raw build or Logcat output by default.
12. Do not expose secret environment values.
13. Use argv-based process execution.
14. Preserve cross-platform behavior and avoid shell assumptions.
15. Keep rendering event-driven.
16. Add tests for protocol changes and migrations.
17. Add an ADR for expensive-to-reverse architectural changes.
18. Keep brand-specific strings centralized.
19. Use only the final `DexDeck`, `dexdeck`, `.dexdeck`, and `DEXDECK_*` identifiers in new work.
20. Treat Lazuli as the default theme and use semantic tokens rather than hardcoded widget colors.
21. Do not introduce Google/Android robot imagery, Dragon Ball references, a mascot, crypto aesthetics, or literal playing-card branding.
22. Do not require bundled fonts or Nerd Font glyphs.
23. Prefer a working, correct CLI foundation before polishing the TUI.
24. Build and validate the Gradle model bridge before implementing broad UI features.

When uncertain, choose the solution that is more private, predictable, bounded, and testable.

---

## 36. Architecture decision records to create first

The implementation should create ADRs for at least:

1. Rust + Ratatui + Tokio architecture
2. Unidirectional state/effect model
3. Embedded Java 17 Gradle bridge
4. Versioned JSONL bridge protocol
5. OS cache and optional shared TOML configuration
6. Zero-network and zero-telemetry policy
7. One Gradle operation per root
8. Byte-bounded Logcat ring buffer
9. Direct argv-based custom command execution
10. One project and one active device per process in v0.1
11. DexDeck public identity, Deckmark constraints, and Lazuli semantic theme

---

## 37. Final implementation order

The first serious implementation work should occur in this order:

1. Establish workspace, CI, terminal guard, and event architecture.
2. Define normalized project-model types and protocol schemas.
3. Build the Java Gradle bridge.
4. Test the bridge against representative fixture projects.
5. Build a minimal Rust CLI that consumes the model.
6. Add cache loading, fingerprint invalidation, and degraded mode.
7. Add SDK/ADB/device abstractions.
8. Add build/install/launch/test functionality.
9. Add the structured Logcat pipeline.
10. Build the full interactive dashboard on proven core services.
11. Harden, benchmark, audit privacy, and package.

A polished TUI over an unreliable Gradle model is not acceptable. The project model, process supervision, and bounded streaming architecture are the foundation of the product.

---

## 38. Final summary

DexDeck is a complete rewrite built around a simple proposition: Android developers should be able to use a lightweight terminal-native control surface for the routine development loop without accepting Android Studio’s resource footprint.

The application is local-first, private by construction, explicit in its behavior, and optimized around the actual Android toolchain rather than attempting to replace every IDE feature.

The defining implementation traits are:

- Rust native executable
- Ratatui interactive dashboard
- Shared CLI/TUI application core
- Embedded Java Gradle bridge
- AGP-backed variant modeling
- Cached startup with safe invalidation
- One active Gradle operation and one active device per instance
- Structured, bounded, package-aware Logcat
- Local unit and instrumentation testing
- Zero telemetry
- Zero direct network requests
- No background daemon
- No automatic project mutation
- Apache-2.0 open source with DCO
- Final DexDeck public identity with centralized package and path identifiers
- Lazuli default theme and semantic color system
- Deckmark geometric logo system with independent, non-Google branding

This specification should be used as the source of truth for planning, issue creation, architecture, design, brand assets, code review, release engineering, and AI-assisted implementation.
