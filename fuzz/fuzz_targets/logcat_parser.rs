#![no_main]

use dexdeck_android::LogcatParser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut parser = LogcatParser::default();
    for chunk in data.chunks(31) {
        let _ = parser.push(chunk);
    }
    let _ = parser.finish();
    assert!(parser.stats().physical_lines <= data.len() as u64 + 1);
});
