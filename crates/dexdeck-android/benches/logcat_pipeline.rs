use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use dexdeck_android::LogcatParser;
use dexdeck_core::ByteBoundedLogBuffer;

fn parser_pipeline(criterion: &mut Criterion) {
    let line = b"2026-07-16 12:01:02.123456 UTC 10123 42 43 I App: benchmark message\n";
    let input = line.repeat(4096);
    let mut group = criterion.benchmark_group("logcat");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("parse_threadtime", |bencher| {
        bencher.iter(|| {
            let mut parser = LogcatParser::new();
            let records = parser.push(std::hint::black_box(&input));
            std::hint::black_box(records);
        });
    });
    group.bench_function("parse_and_buffer", |bencher| {
        bencher.iter(|| {
            let mut parser = LogcatParser::new();
            let mut buffer = ByteBoundedLogBuffer::default();
            for record in parser.push(std::hint::black_box(&input)) {
                let _ = buffer.push(record);
            }
            std::hint::black_box(buffer.stats());
        });
    });
    group.finish();
}

criterion_group!(benches, parser_pipeline);
criterion_main!(benches);
