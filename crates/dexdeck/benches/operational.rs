use clap::Parser;
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use dexdeck::Cli;
use dexdeck_core::{ByteBoundedLogBuffer, DiagnosticNormalizer, JobRequest, JobScheduler};
use dexdeck_protocol::{JobId, JobKind, JobRecord, JobState, LogPriority, LogRecord, ProjectModel};
use dexdeck_tui::{VirtualList, fuzzy_actions};

fn operational(criterion: &mut Criterion) {
    criterion.bench_function("startup/cold_cli_parse", |bencher| {
        bencher.iter(|| Cli::try_parse_from(["dexdeck", "--project", ".", "project", "inspect"]));
    });

    let model = serde_json::to_vec(&ProjectModel::empty("/workspace".into()))
        .unwrap_or_else(|error| panic!("benchmark model: {error}"));
    criterion.bench_function("startup/warm_model_load", |bencher| {
        bencher.iter(|| serde_json::from_slice::<ProjectModel>(std::hint::black_box(&model)));
    });

    criterion.bench_function("idle/input_palette_latency", |bencher| {
        bencher.iter(|| fuzzy_actions(std::hint::black_box("open log")));
    });

    criterion.bench_function("tasks/virtualized_100k", |bencher| {
        bencher.iter(|| {
            let mut list = VirtualList::default();
            list.select(75_000, 100_000, 40);
            std::hint::black_box(list.visible(100_000, 40));
        });
    });

    let output = b"e: /workspace/App.kt: (12, 4): unresolved reference\n".repeat(4096);
    let mut output_group = criterion.benchmark_group("build_output");
    output_group.throughput(Throughput::Bytes(output.len() as u64));
    output_group.bench_function("normalize", |bencher| {
        bencher.iter_batched(
            DiagnosticNormalizer::new,
            |mut parser| parser.push(std::hint::black_box(&output)),
            BatchSize::SmallInput,
        );
    });
    output_group.finish();

    criterion.bench_function("logcat/bounded_32mib", |bencher| {
        bencher.iter_batched(
            ByteBoundedLogBuffer::default,
            |mut buffer| {
                for index in 0..1_000 {
                    let _ = buffer.push(log_record(index));
                }
                std::hint::black_box(buffer.stats());
            },
            BatchSize::SmallInput,
        );
    });

    criterion.bench_function("jobs/cancellation", |bencher| {
        bencher.iter_batched(
            || {
                let mut scheduler = JobScheduler::new(64 * 1024)
                    .unwrap_or_else(|error| panic!("benchmark scheduler: {error}"));
                let id = JobId("benchmark".into());
                scheduler
                    .submit(JobRequest {
                        record: job_record(id.clone()),
                        mutating_gradle_root: Some("/workspace".into()),
                    })
                    .unwrap_or_else(|error| panic!("benchmark job: {error}"));
                scheduler
                    .mark_running(&id)
                    .unwrap_or_else(|error| panic!("benchmark running job: {error}"));
                (scheduler, id)
            },
            |(mut scheduler, id)| std::hint::black_box(scheduler.cancel(&id)),
            BatchSize::SmallInput,
        );
    });
}

fn log_record(index: u32) -> LogRecord {
    LogRecord {
        timestamp: "2026-07-16 12:00:00.000000 UTC".into(),
        process_id: 42,
        thread_id: index,
        user_id: Some(10_123),
        priority: LogPriority::Info,
        tag: "Bench".into(),
        message: "bounded benchmark message".repeat(4),
        package: Some("dev.dexdeck.bench".into()),
        process: Some("dev.dexdeck.bench".into()),
        continuation: false,
        crash_boundary: false,
        group_id: None,
        marker: None,
        truncated: false,
    }
}

fn job_record(id: JobId) -> JobRecord {
    JobRecord {
        id,
        kind: JobKind::Build,
        state: JobState::Queued,
        project_identity: "benchmark".into(),
        module: Some(":app".into()),
        variant: Some("debug".into()),
        device: None,
        command_summary: vec!["assembleDebug".into()],
        started_at: "2026-07-16T12:00:00Z".into(),
        finished_at: None,
        duration_ms: None,
        exit_code: None,
        diagnostics: Vec::new(),
    }
}

criterion_group!(benches, operational);
criterion_main!(benches);
