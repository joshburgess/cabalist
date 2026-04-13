//! Error recovery tests for the cabalist parser.
//!
//! These tests verify that the parser handles malformed, unusual, and hostile
//! input without panicking. The parser should always produce *some* output,
//! even if the input is garbage.

use cabalist_parser::parse;

// ---------------------------------------------------------------------------
// No-panic tests: the parser must never panic on any input.
// ---------------------------------------------------------------------------

#[test]
fn no_panic_on_garbage() {
    let long_a = "a".repeat(10000);
    let many_newlines = "\n".repeat(10000);

    let inputs: Vec<&str> = vec![
        "",
        "\n",
        "\n\n\n",
        "   ",
        "\t\t\t",
        "::::",
        "name:",
        "name: ",
        "name",
        ": value",
        "library\n",
        "library\n  \n",
        "if\n",
        "if flag(dev)\n",
        "else\n",
        "else\n  foo: bar\n",
        "import: \n",
        "import:\n",
        "\x00\x01\x02",
        "name: foo\n\x00version: bar\n",
        &long_a,
        &many_newlines,
        "library\nlibrary\n",
        "if flag(dev)\n  if flag(test)\n    if os(linux)\n",
        "name: foo\n  bar\n    baz\n      quux\n",
        "-- just a comment",
        "name: foo",
        "executable \n",
        "name: foo\rversion: bar\r",
        "name: foo\r\nversion: bar\r\n",
        "name: foo\n\r\nversion: bar\n",
        // Additional edge cases
        ":",
        "  :",
        "  :  ",
        "flag dev\n  default: True\n",
        "common\n  ghc-options: -Wall\n",
        "source-repository head\n  type: git\n  location: https://example.com\n",
        // Lots of colons
        "a:b:c:d:e\n",
        // Field name that looks like section header
        "library: foo\n",
        // Indentation chaos
        "  name: foo\n    version: bar\n  license: MIT\n",
        // Only whitespace on lines
        "  \n  \n  \n",
        // Tab-heavy
        "\tname:\tfoo\n\tversion:\tbar\n",
        // Very deep nesting
        "library\n  if flag(a)\n    if flag(b)\n      if flag(c)\n        if flag(d)\n          ghc-options: -Wall\n",
        // Missing value after colon with continuation
        "name:\n  \n  foo\n",
        // Section with only comments
        "library\n  -- a comment\n  -- another\n",
        // Conditional with else
        "library\n  if flag(dev)\n    ghc-options: -O0\n  else\n    ghc-options: -O2\n",
        // Consecutive conditionals
        "library\n  if os(windows)\n    build-depends: Win32\n  if os(linux)\n    build-depends: unix\n",
    ];

    for (i, input) in inputs.iter().enumerate() {
        let result = std::panic::catch_unwind(|| parse(input));
        assert!(
            result.is_ok(),
            "Parser panicked on input #{i}: {:?}",
            &input[..input.len().min(80)]
        );
    }
}

#[test]
fn render_never_panics_after_parse() {
    let long_a = "a".repeat(10000);
    let many_newlines = "\n".repeat(10000);

    let inputs: Vec<&str> = vec![
        "",
        "\n",
        "\n\n\n",
        "   ",
        "\t\t\t",
        "::::",
        "name:",
        "name: ",
        "name",
        ": value",
        "library\n",
        "library\n  \n",
        "if\n",
        "if flag(dev)\n",
        "else\n",
        "else\n  foo: bar\n",
        "import: \n",
        "import:\n",
        "\x00\x01\x02",
        "name: foo\n\x00version: bar\n",
        &long_a,
        &many_newlines,
        "library\nlibrary\n",
        "-- just a comment",
        "name: foo",
        "executable \n",
        "name: foo\r\nversion: bar\r\n",
    ];

    for (i, input) in inputs.iter().enumerate() {
        let parse_result = std::panic::catch_unwind(|| parse(input));
        if let Ok(result) = parse_result {
            let render_result = std::panic::catch_unwind(move || result.cst.render());
            assert!(
                render_result.is_ok(),
                "render() panicked after parsing input #{i}: {:?}",
                &input[..input.len().min(80)]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Partial recovery tests
// ---------------------------------------------------------------------------

#[test]
fn partial_parse_recovers() {
    // First field is fine, second is garbage, third is fine.
    let source = "name: foo\n!!garbage!!\nversion: 0.1\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert!(!rendered.is_empty(), "Rendered output should not be empty");
}

#[test]
fn malformed_conditional_recovery() {
    let source = "library\n  build-depends: base\n  if\n  ghc-options: -Wall\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(
        rendered, source,
        "Round-trip should still work for malformed conditional"
    );
}

#[test]
fn stray_operator_in_conditional_no_hang() {
    // Regression: the condition-expression lexer used to loop forever on
    // `if` followed by a stray `&`, `|`, or `=` because those bytes were
    // guarded for their doubled forms (`&&`, `||`, `==`) but excluded from
    // the word scanner, so `pos` never advanced.
    for source in [
        "if&",
        "if|",
        "if=",
        "if &\n",
        "if |\n",
        "if =x\n",
        "elif&",
        "library\n  if &\n    ghc-options: -Wall\n",
    ] {
        let result = parse(source);
        // Must round-trip even when the input is malformed.
        assert_eq!(
            result.cst.render(),
            source,
            "Stray operator should round-trip: {source:?}"
        );
    }
}

#[test]
fn duplicate_sections_parsed() {
    let source = "library\n  exposed-modules: Foo\n\nlibrary\n  exposed-modules: Bar\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source, "Duplicate sections should round-trip");
}

#[test]
fn standalone_else_no_crash() {
    let source = "library\n  else\n    ghc-options: -Wall\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert!(
        !rendered.is_empty(),
        "Standalone else should produce output"
    );
}

#[test]
fn empty_section_name() {
    let source = "executable \n  main-is: Main.hs\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(
        rendered, source,
        "Section with trailing space should round-trip"
    );
}

// ---------------------------------------------------------------------------
// Large file / stack overflow tests
// ---------------------------------------------------------------------------

#[test]
fn large_file_no_stack_overflow() {
    let mut source = String::from("cabal-version: 3.0\nname: big\nversion: 0.1\n\n");
    // 1000 sections with 10 fields each.
    for i in 0..1000 {
        source.push_str(&format!("executable exe-{i}\n"));
        for j in 0..10 {
            source.push_str(&format!("  field-{j}: value-{j}\n"));
        }
        source.push('\n');
    }
    let result = parse(&source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn many_fields_single_section() {
    let mut source = String::from("library\n");
    for i in 0..500 {
        source.push_str(&format!("  field-{i}: value-{i}\n"));
    }
    let result = parse(&source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn deeply_nested_conditionals() {
    let mut source = String::from("library\n");
    let depth = 50;
    for i in 0..depth {
        let indent = "  ".repeat(i + 1);
        source.push_str(&format!("{indent}if flag(f{i})\n"));
    }
    let deep_indent = "  ".repeat(depth + 1);
    source.push_str(&format!("{deep_indent}ghc-options: -Wall\n"));

    let result = parse(&source);
    // Should not stack overflow; render should produce something.
    let rendered = result.cst.render();
    assert!(!rendered.is_empty());
}

// ---------------------------------------------------------------------------
// Unicode / encoding tests
// ---------------------------------------------------------------------------

#[test]
fn unicode_preservation() {
    let source = "name: project\nsynopsis: A project\nversion: 0.1\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn unicode_field_values() {
    // Emoji, CJK, accented chars.
    let inputs = [
        "description: This is a project\nversion: 0.1\n",
        "synopsis: Bibliothek\nversion: 0.1\n",
        "author: Name <name@example.com>\nversion: 0.1\n",
    ];
    for input in &inputs {
        let result = parse(input);
        let rendered = result.cst.render();
        assert_eq!(rendered, *input);
    }
}

// ---------------------------------------------------------------------------
// Fuzz-like byte mutation tests
// ---------------------------------------------------------------------------

/// Take a known-good `.cabal` source, randomly mutate a few bytes, and verify
/// no panic.
#[test]
fn byte_mutations_no_panic() {
    let base = "\
cabal-version: 3.0
name: test-pkg
version: 0.1.0.0
synopsis: A test package

library
  exposed-modules: Foo
  build-depends:
    base >=4.14 && <5
  default-language: GHC2021

executable my-exe
  main-is: Main.hs
  build-depends: base, test-pkg
  hs-source-dirs: app
  default-language: GHC2021
";

    let bytes = base.as_bytes().to_vec();
    // Deterministic mutations: try flipping each byte position in small
    // samples throughout the file.
    let positions: Vec<usize> = (0..bytes.len()).step_by(7).collect();
    let mutations: &[u8] = &[0, b'\n', b':', b' ', b'\t', 0xFF, b'\\', b'"'];

    for &pos in &positions {
        for &mutation in mutations {
            let mut mutated = bytes.clone();
            mutated[pos] = mutation;
            // The mutated bytes might not be valid UTF-8; skip those.
            if let Ok(s) = std::str::from_utf8(&mutated) {
                let result = std::panic::catch_unwind(|| parse(s));
                assert!(
                    result.is_ok(),
                    "Parser panicked on mutation at byte {pos} -> {mutation:#04x}"
                );
            }
        }
    }
}

/// Take a known-good `.cabal` file and truncate it at various positions.
#[test]
fn truncation_no_panic() {
    let base = "\
cabal-version: 3.0
name: test-pkg
version: 0.1.0.0
synopsis: A test package

library
  exposed-modules: Foo Bar
  build-depends:
    base >=4.14 && <5
    text >=2.0
  default-language: GHC2021

executable my-exe
  main-is: Main.hs
  build-depends: base, test-pkg
  hs-source-dirs: app
  default-language: GHC2021
";

    // Truncate at every position.
    for i in 0..base.len() {
        // Only truncate at valid UTF-8 boundaries.
        if base.is_char_boundary(i) {
            let truncated = &base[..i];
            let result = std::panic::catch_unwind(|| parse(truncated));
            assert!(
                result.is_ok(),
                "Parser panicked on truncation at byte {i}: {:?}",
                &truncated[truncated.len().saturating_sub(20)..]
            );
        }
    }
}

/// Verify that truncated files also survive render() without panic.
#[test]
fn truncation_render_no_panic() {
    let base = "\
cabal-version: 3.0
name: test-pkg
version: 0.1.0.0

library
  build-depends: base >=4.14
  ghc-options: -Wall
";

    for i in 0..base.len() {
        if base.is_char_boundary(i) {
            let truncated = &base[..i];
            if let Ok(result) = std::panic::catch_unwind(|| parse(truncated)) {
                let render_result = std::panic::catch_unwind(move || result.cst.render());
                assert!(
                    render_result.is_ok(),
                    "render() panicked after parsing truncated input at byte {i}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Line ending tests
// ---------------------------------------------------------------------------

#[test]
fn crlf_no_panic() {
    let source = "name: foo\r\nversion: bar\r\nlibrary\r\n  exposed-modules: Foo\r\n";
    let result = parse(source);
    let rendered = result.cst.render();
    // May or may not perfectly round-trip CRLF, but must not panic.
    assert!(!rendered.is_empty());
}

#[test]
fn mixed_line_endings_no_panic() {
    let source = "name: foo\n\r\nversion: bar\r\nlicense: MIT\n";
    let result = parse(source);
    let _ = result.cst.render();
}

#[test]
fn no_trailing_newline() {
    let source = "name: foo\nversion: 0.1";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(
        rendered, source,
        "File without trailing newline should round-trip"
    );
}

// ---------------------------------------------------------------------------
// Edge case field/section formats
// ---------------------------------------------------------------------------

#[test]
fn field_with_only_colon() {
    let source = "name:\nversion:\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn field_with_extra_spacing() {
    let source = "name:    foo\nversion:    0.1\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn section_with_no_fields() {
    let source = "library\n\nexecutable foo\n  main-is: Main.hs\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn only_blank_lines() {
    let source = "\n\n\n\n\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn only_whitespace() {
    let source = "   \n   \n   \n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn comment_without_trailing_newline() {
    let source = "-- just a comment";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn import_field() {
    let source = "library\n  import: warnings\n  exposed-modules: Foo\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}

#[test]
fn empty_import() {
    let source = "library\n  import:\n  exposed-modules: Foo\n";
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(rendered, source);
}
