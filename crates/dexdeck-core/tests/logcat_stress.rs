use dexdeck_core::{ByteBoundedLogBuffer, CompiledLogFilter, LogFilterSpec, MIN_LOG_BUFFER_BYTES};
use dexdeck_protocol::{LogPriority, LogRecord, LogTextSearch};

fn record(index: u64) -> LogRecord {
    LogRecord {
        timestamp: format!(
            "2026-07-16 12:{:02}:{:02}.{:06}",
            index / 60 % 60,
            index % 60,
            index % 1_000_000
        ),
        process_id: u32::try_from(index % 512).unwrap_or_default(),
        thread_id: u32::try_from(index % 1024).unwrap_or_default(),
        user_id: Some(10123),
        priority: if index.is_multiple_of(100) {
            LogPriority::Error
        } else {
            LogPriority::Info
        },
        tag: if index.is_multiple_of(2) {
            "App".into()
        } else {
            "Worker".into()
        },
        message: format!("bounded stress record {index} {}", "x".repeat(160)),
        package: Some("com.example".into()),
        process: Some(if index.is_multiple_of(3) {
            "com.example:sync".into()
        } else {
            "com.example".into()
        }),
        continuation: false,
        crash_boundary: false,
        group_id: Some(index),
        marker: None,
        truncated: false,
    }
}

#[test]
#[ignore = "bounded release-mode CI stress"]
fn logcat_stress_keeps_memory_bounded_and_filtering_responsive()
-> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = ByteBoundedLogBuffer::new(MIN_LOG_BUFFER_BYTES)?;
    for index in 0..250_000 {
        let _ = buffer.push(record(index));
        assert!(buffer.stats().buffered_bytes <= MIN_LOG_BUFFER_BYTES);
    }
    assert!(buffer.stats().dropped_entries > 0);
    let snapshot = buffer.snapshot();
    assert!(
        snapshot
            .windows(2)
            .all(|entries| entries[0].sequence < entries[1].sequence)
    );
    let filter = CompiledLogFilter::compile(LogFilterSpec {
        minimum_priority: Some(LogPriority::Error),
        include_tags: vec!["App".into(), "Worker".into()],
        text_search: Some(LogTextSearch::Regex("stress record [0-9]+".into())),
        ..LogFilterSpec::default()
    })?;
    assert!(
        snapshot
            .iter()
            .filter(|entry| filter.matches(&entry.record))
            .count()
            > 0
    );
    Ok(())
}
