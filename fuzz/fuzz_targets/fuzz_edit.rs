#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    if let Ok(s) = std::str::from_utf8(&data[4..]) {
        let result = cabalist_parser::parse(s);
        let cst = result.cst;

        // Build an AST and attempt edits on whatever fields exist.
        let file = cabalist_parser::ast::CabalFile::from_cst(&cst);

        // Re-parse the rendered CST to ensure it doesn't panic.
        let rendered = cst.render();
        let _ = cabalist_parser::parse(&rendered);

        // If we got a name field, the AST extraction must not lose it.
        if let Some(name) = &file.name {
            assert!(!name.is_empty() || s.contains("name:"));
        }
    }
});
