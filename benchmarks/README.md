# Reproducible benchmarks

Run `scripts/benchmark.sh [output-directory]` from a clean checkout. The script
records the Git revision, dirty state, Rust/Cargo versions, target, operating
system, CPU, memory, and UTC timestamp beside Criterion output. Compare runs
only when release profile, fixture revision, and hardware metadata match.

The suite covers cold CLI parsing, warm model loading, idle/input paths,
virtualized task lists, build-output normalization, bounded Logcat storage, and
job cancellation. `dexdeck-android` separately benchmarks the complete Logcat
parser pipeline. Startup wall-clock and idle wakeups should also be recorded by
the platform smoke jobs because process startup and terminal scheduling cannot
be measured honestly inside a Criterion process.

Do not publish results from thermally throttled, battery-saving, virtualized, or
otherwise contended machines without recording that condition in `notes.txt`.
