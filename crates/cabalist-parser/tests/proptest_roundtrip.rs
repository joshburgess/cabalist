//! Property-based round-trip tests for the cabalist parser.
//!
//! The core invariant: `render(parse(source)) == source` for any valid
//! `.cabal`-like input. Uses `proptest` to generate random `.cabal` files and
//! verify byte-identical round-tripping.

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies for generating random `.cabal`-like content
// ---------------------------------------------------------------------------

/// Generate a random valid field name (lowercase letters and hyphens).
fn arb_field_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,20}".prop_filter("no trailing hyphen", |s| !s.ends_with('-'))
}

/// Generate a random field value (no newlines, printable ASCII).
fn arb_field_value() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 .>=<^!&|,(){}_/-]{0,60}"
}

/// Generate a random comment.
fn arb_comment() -> impl Strategy<Value = String> {
    "-- [a-zA-Z0-9 ]{0,40}"
}

/// Generate a random simple field line: "name: value\n"
fn arb_field_line() -> impl Strategy<Value = String> {
    (arb_field_name(), arb_field_value()).prop_map(|(name, val)| {
        if val.is_empty() {
            format!("{name}:\n")
        } else {
            format!("{name}: {val}\n")
        }
    })
}

/// Generate a random section (library, executable, etc).
fn arb_section() -> impl Strategy<Value = String> {
    let keywords = prop_oneof![
        Just("library".to_string()),
        Just("executable".to_string()),
        Just("test-suite".to_string()),
        Just("benchmark".to_string()),
        Just("common".to_string()),
        Just("flag".to_string()),
        Just("source-repository".to_string()),
    ];
    let name = prop_oneof![
        Just("".to_string()),
        "[a-z][a-z0-9-]{0,10}".prop_filter("no trailing hyphen", |s| !s.ends_with('-')),
    ];
    let fields = prop::collection::vec(arb_field_line().prop_map(|f| format!("  {f}")), 0..5);

    (keywords, name, fields).prop_map(|(kw, name, fields)| {
        let header = if name.is_empty() || kw == "library" {
            format!("{kw}\n")
        } else {
            format!("{kw} {name}\n")
        };
        let body = fields.join("");
        format!("{header}{body}")
    })
}

/// Generate a complete random `.cabal` file.
fn arb_cabal_file() -> impl Strategy<Value = String> {
    let header_fields = prop::collection::vec(arb_field_line(), 1..5);
    let sections = prop::collection::vec(arb_section(), 0..4);

    (header_fields, sections).prop_map(|(fields, sections)| {
        let mut result = String::new();
        // Start with cabal-version (required to be first).
        result.push_str("cabal-version: 3.0\n");
        for f in fields {
            result.push_str(&f);
        }
        for s in sections {
            result.push('\n');
            result.push_str(&s);
        }
        result
    })
}

/// Generate a multi-line field value (field name + indented continuation lines).
fn arb_multiline_field() -> impl Strategy<Value = String> {
    let first_value = arb_field_value();
    let continuations = prop::collection::vec(
        prop::collection::vec(
            prop::sample::select(vec![
                "foo", "bar", "baz", "qux", "Data.Map", "base", ">=4.14",
                "text", "aeson", "^>=2.2", "containers", "-Wall", "-Wcompat",
            ]),
            1..4,
        )
        .prop_map(|words| format!("    {}\n", words.join(" "))),
        1..4,
    );
    (arb_field_name(), first_value, continuations).prop_map(|(name, first, conts)| {
        let mut s = if first.is_empty() {
            format!("{name}:\n")
        } else {
            format!("{name}: {first}\n")
        };
        for c in conts {
            s.push_str(&c);
        }
        s
    })
}

/// Generate a file containing a conditional block.
fn arb_conditional_file() -> impl Strategy<Value = String> {
    let flag_name =
        "[a-z][a-z0-9]{0,8}".prop_filter("non-empty", |s| !s.is_empty());
    let fields = prop::collection::vec(
        arb_field_line().prop_map(|f| format!("    {f}")),
        1..3,
    );

    (flag_name, fields).prop_map(|(flag, fields)| {
        let mut result = String::new();
        result.push_str("cabal-version: 3.0\n");
        result.push_str("name: cond-test\n");
        result.push_str("version: 0.1\n");
        result.push('\n');
        result.push_str("library\n");
        result.push_str(&format!("  if flag({flag})\n"));
        for f in fields {
            result.push_str(&f);
        }
        result
    })
}

/// Generate a file that is entirely comments.
fn arb_comments_only() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_comment().prop_map(|c| format!("{c}\n")), 1..6)
        .prop_map(|lines| lines.join(""))
}

/// Generate a file with blank lines interspersed between sections.
fn arb_file_with_blank_lines() -> impl Strategy<Value = String> {
    let sections = prop::collection::vec(arb_section(), 1..4);
    sections.prop_map(|secs| {
        let mut result = String::from("cabal-version: 3.0\nname: blanks\nversion: 0.1\n");
        for s in secs {
            // Insert 1-3 blank lines before each section.
            result.push('\n');
            result.push_str(&s);
        }
        result
    })
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn round_trip_arbitrary_cabal(source in arb_cabal_file()) {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn parse_never_panics(source in "[ -~\n\t]{0,200}") {
        // Any printable ASCII + newlines + tabs should not panic.
        let _ = cabalist_parser::parse(&source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn round_trip_single_field(name in arb_field_name(), value in arb_field_value()) {
        let source = if value.is_empty() {
            format!("{name}:\n")
        } else {
            format!("{name}: {value}\n")
        };
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn round_trip_with_comments(
        field in arb_field_line(),
        comment in arb_comment()
    ) {
        let source = format!("{comment}\n{field}");
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn round_trip_multiline_field(source in arb_multiline_field()) {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn round_trip_conditional(source in arb_conditional_file()) {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn round_trip_blank_lines(source in arb_file_with_blank_lines()) {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn round_trip_comments_only(source in arb_comments_only()) {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn round_trip_empty_and_whitespace(source in "[ \t\n]{0,100}") {
        let result = cabalist_parser::parse(&source);
        let rendered = result.cst.render();
        prop_assert_eq!(rendered, source);
    }
}
