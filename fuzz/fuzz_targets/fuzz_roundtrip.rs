#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let result = cabalist_parser::parse(s);
        // An unmodified CST must always render back to the original source.
        assert_eq!(result.cst.render(), s);
    }
});
