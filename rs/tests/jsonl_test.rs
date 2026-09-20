// In-language tests for the Rust port.
//
// Parse cases expressible as `input -> JSON` live in the shared
// test/spec/*.tsv fixtures instead, so they run in EVERY runtime (see
// ../../test/AGENTS.md). What is left here is what a fixture cannot
// state: the crate's API surface, error metadata, the layering contract,
// scale, and the shared default parser under threads. Mirrors
// go/jsonl_test.go and ts/test/jsonl.test.ts case for case, as far as the
// engine API allows.

mod common;

use common::{plain, records};
use serde_json::json;
use tabnas::{Options, Tabnas, Value};
use tabnas_jsonl::{jsonl, make, parse, register_jsonl_grammar, JsonlError, VERSION};

fn must_parse(src: &str) -> Vec<serde_json::Value> {
    match parse(src) {
        Ok(value) => records(&value),
        Err(error) => panic!("parse {src:?}: {error}"),
    }
}

// --- API -----------------------------------------------------------------

#[test]
fn parse_returns_one_value_per_line() {
    assert_eq!(
        must_parse("{\"a\":1}\n{\"b\":2}"),
        vec![json!({"a": 1}), json!({"b": 2})]
    );
}

#[test]
fn a_single_record_still_yields_an_array() {
    let value = parse(r#"{"a":1}"#).expect("parses");
    assert!(
        matches!(value, Value::Array(_)),
        "expected an array, got {value:?}"
    );
    assert_eq!(records(&value).len(), 1);
}

#[test]
fn repeated_parse_calls_reuse_one_engine_without_leaking_state() {
    let first = must_parse("{\"a\":1}\n{\"b\":2}");
    let second = must_parse(r#"{"c":3}"#);
    assert_eq!(first, vec![json!({"a": 1}), json!({"b": 2})]);
    assert_eq!(second, vec![json!({"c": 3})]);
}

#[test]
fn each_parse_returns_a_fresh_array() {
    // The engine's `lex.emptyResult` would return one shared instance;
    // nothing here does, so two parses of the same source are two arrays,
    // for the zero-record document as much as for any other.
    for src in ["1", "\n"] {
        let Value::Array(first) = parse(src).expect("parses") else {
            panic!("an array")
        };
        let Value::Array(second) = parse(src).expect("parses") else {
            panic!("an array")
        };
        assert!(
            !std::sync::Arc::ptr_eq(&first, &second),
            "results must not be a shared instance"
        );
        assert_eq!(first, second);
    }
}

#[test]
fn make_builds_independent_instances() {
    let one = make();
    let two = make();
    for parser in [&one, &two] {
        let value = parser.parse("1\n2").expect("parses");
        assert_eq!(records(&value), vec![json!(1), json!(2)]);
    }
    // Configuring one leaves the other alone.
    let mut one = one;
    one.set_options(|options: &mut Options| options.comment.lex = true)
        .expect("options apply");
    assert!(one.parse("1 // note").is_ok());
    assert!(two.parse("1 // note").is_err());
}

#[test]
fn make_and_parse_agree() {
    // `parse` goes through `make`, which goes through the plugin, so the
    // two construction paths cannot drift.
    let src = "{\"a\":[1,2]}\n\"s\"";
    let direct = parse(src).expect("parses");
    let instance = make().parse(src).expect("parses");
    assert_eq!(plain(&direct), plain(&instance));
}

#[test]
fn version_is_semver_shaped() {
    assert!(
        VERSION.split('.').count() >= 3,
        "VERSION {VERSION:?} does not look like semver"
    );
}

#[test]
fn jsonl_error_is_the_engine_error_type() {
    // A type-level assertion: the alias IS the engine's error, so a caller
    // can read its `code`, `row` and `col` without naming the engine.
    fn takes_engine_error(_: &tabnas::TabnasError) {}
    let error: JsonlError = parse("{bad").unwrap_err();
    takes_engine_error(&error);
    assert!(!error.code.is_empty(), "expected an error code");
}

// --- Layering on tabnas-json ----------------------------------------------

#[test]
fn json_then_jsonl_is_the_supported_composition() {
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("json installs");

    // The strict-JSON rules are reused, not redefined: exactly two rules
    // are added on top of the five that arrive with the JSON grammar.
    let before = parser.rule_names();
    assert_eq!(before, ["val", "map", "list", "pair", "elem"]);

    jsonl(&mut parser).expect("jsonl installs");
    let after = parser.rule_names();
    assert_eq!(
        after,
        ["val", "map", "list", "pair", "elem", "jsonl", "record"]
    );

    let value = parser.parse("{\"a\":1}\n2").expect("parses");
    assert_eq!(records(&value), vec![json!({"a": 1}), json!(2)]);
}

#[test]
fn installing_on_a_bare_engine_reports_the_missing_base_grammar() {
    let mut parser = Tabnas::new();
    let error =
        jsonl(&mut parser).expect_err("expected an error when the strict-JSON grammar is absent");
    assert!(
        error
            .to_string()
            .contains("strict-JSON grammar must be installed first"),
        "expected a named error, got {error:?}"
    );
    // And nothing was installed: the failure is a refusal, not a partial
    // install.
    assert!(parser.rule_names().is_empty());
}

#[test]
fn a_relaxed_value_grammar_is_rejected_not_silently_accepted() {
    // A `val` rule alone does not make a base strict: a relaxed grammar
    // has one too, and layering on it would accept `{a:1}` as a record.
    // The plugin checks the lexer options that decide record content
    // instead. Relax one of the three back to a non-JSON setting.
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("json installs");
    parser
        .set_options(|options: &mut Options| options.text.lex = true)
        .expect("options apply");

    let error =
        jsonl(&mut parser).expect_err("expected jsonl to reject a non-strict value grammar");
    let message = error.to_string();
    assert!(
        message.contains("not strict JSON") && message.contains("text.lex"),
        "error should name the offending option, got {message:?}"
    );
    // The other two options were fine, so they are not named.
    assert!(!message.contains("comment.lex"), "{message}");
    assert!(!message.contains("string.chars"), "{message}");
}

#[test]
fn the_error_names_every_relaxed_option() {
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("json installs");
    parser
        .set_options(|options: &mut Options| {
            options.text.lex = true;
            options.comment.lex = true;
            options.string.chars = "\"'".into();
        })
        .expect("options apply");

    let message = jsonl(&mut parser).expect_err("rejected").to_string();
    assert!(
        message.contains("text.lex, comment.lex, string.chars"),
        "expected all three named in order, got {message:?}"
    );
}

#[test]
fn json_applied_after_jsonl_filters_the_document_rules_out() {
    // The reason the order is enforced: `tabnas_json` narrows
    // `rule.include` to `json`, so a JSONL document stops parsing even
    // though the rules are still installed.
    let mut parser = make();
    assert!(parser.parse("1\n2").is_ok());
    tabnas_json::json(&mut parser).expect("json re-installs");
    assert!(
        parser.parse("1\n2").is_err(),
        "with include narrowed back to json, the jsonl alternates are gone"
    );
}

#[test]
fn setting_options_afterwards_keeps_the_rule_set_and_the_behaviour() {
    // Mirrors the half of the re-application case the engine API can
    // express: options set after the plugin neither duplicate rules nor
    // change parse behaviour.
    let mut parser = make();
    let before = parser.rule_names();

    parser
        .set_options(|options: &mut Options| options.comment.lex = true)
        .expect("options apply");
    assert_eq!(
        parser.rule_names(),
        before,
        "rule set changed after options"
    );
    let value = parser.parse("{\"a\":1}\n{\"b\":2}").expect("parses");
    assert_eq!(records(&value), vec![json!({"a": 1}), json!({"b": 2})]);
}

#[test]
fn the_strict_json_rules_are_reused_not_redefined() {
    // `register_jsonl_grammar` adds exactly the two document rules; the
    // five JSON rules must already be there and must survive. The
    // options are applied by hand here, the way the TypeScript case does
    // it, so this is the rules-only half on its own.
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("json installs");
    assert_eq!(parser.rule_names(), ["val", "map", "list", "pair", "elem"]);

    // The survivors of the default IGNORE set, by token name: this engine
    // replaces a token set outright, as the Go engine does.
    let ignore: Vec<_> = parser
        .token_set("IGNORE")
        .unwrap_or_default()
        .into_iter()
        .filter(|tin| parser.token_name(*tin) != "#LN")
        .collect();
    parser
        .set_options(|options: &mut Options| {
            options.token_set.insert("IGNORE".into(), ignore);
            options.rule.start = "jsonl".into();
            options.rule.include = "json,jsonl".into();
        })
        .expect("options apply");
    register_jsonl_grammar(&mut parser).expect("rules install");

    assert_eq!(
        parser.rule_names(),
        ["val", "map", "list", "pair", "elem", "jsonl", "record"]
    );
    let value = parser.parse("{\"a\":1}\n2").expect("parses");
    assert_eq!(records(&value), vec![json!({"a": 1}), json!(2)]);
}

#[test]
fn a_record_is_parsed_by_the_inherited_json_value_grammar() {
    // Anything tabnas_json accepts as a value is accepted as a record.
    let src = r#"{"s":"x","n":-1.5e2,"t":true,"f":false,"z":null,"l":[1,[2]],"m":{"k":{}}}"#;
    let want: serde_json::Value = serde_json::from_str(src).expect("test data is JSON");
    assert_eq!(must_parse(src), vec![common::whole(want)]);
}

// --- Errors ---------------------------------------------------------------

#[test]
fn a_bad_record_reports_the_line_it_is_on() {
    let error = parse("{\"a\":1}\n{\"b\":}\n{\"c\":3}").expect_err("expected a parse error");
    assert_eq!(error.row, 2, "error should point at the second record");
    assert!(!error.code.is_empty(), "expected an error code");
}

#[test]
fn line_numbers_survive_many_preceding_records() {
    let mut src: String = (0..50).map(|i| format!("{{\"i\":{i}}}\n")).collect();
    src.push_str("not-json");
    let error = parse(&src).expect_err("expected a parse error");
    assert_eq!(error.row, 51);
}

#[test]
fn a_value_split_across_lines_fails_on_the_line_it_breaks() {
    // The shared oneline.tsv fixture pins only that a split value FAILS;
    // it cannot check where. Without this, a Rust-only regression in the
    // error location would pass the parity suite.
    let error = parse("{\"a\":1}\n{\"b\":\n2}").expect_err("expected a parse error");
    assert_eq!(error.row, 2);
}

#[test]
fn empty_source_is_rejected_blank_lines_are_not() {
    // Empty source is rejected by the strict-JSON base (`lex.empty`
    // false), matching `JSON.parse("")` and `serde_json`. A document of
    // only blank lines is a different case: it holds zero records.
    assert!(parse("").is_err(), "expected empty source to be rejected");
    assert_eq!(must_parse("\n"), Vec::<serde_json::Value>::new());
}

#[test]
fn an_instance_is_reusable_after_a_failure() {
    let parser = make();
    assert!(parser.parse("1").is_ok());
    assert!(parser.parse("{bad").is_err());
    assert!(parser.parse("2\n3").is_ok());
}

// --- Scale ----------------------------------------------------------------

#[test]
fn a_large_document_parses_without_growing_the_rule_stack() {
    // `record` iterates by REPLACE, so depth is constant in the record
    // count. A push-based loop would fail well before this.
    const N: usize = 20_000;
    let src: Vec<String> = (0..N).map(|i| format!("{{\"i\":{i}}}")).collect();
    let out = must_parse(&src.join("\n"));
    assert_eq!(out.len(), N);
    assert_eq!(out[0], json!({"i": 0}));
    assert_eq!(out[N - 1], json!({"i": N - 1}));
}

#[test]
fn deep_nesting_inside_one_record_still_works() {
    // The TypeScript case nests 200 deep. tabnas_json holds this port to
    // serde_json's limit of 127 open containers, so the depth here stays
    // inside it; the limit itself is tabnas_json's to test.
    let depth = 100;
    let src = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let out = must_parse(&src);
    assert_eq!(out.len(), 1);
    let mut node = &out[0];
    let mut seen = 1;
    while let Some(items) = node.as_array().filter(|items| !items.is_empty()) {
        node = &items[0];
        seen += 1;
    }
    assert_eq!(seen, depth);
}

// --- The shared default parser ---------------------------------------------

#[test]
fn the_shared_default_parser_takes_concurrent_callers() {
    // `parse` builds its engine once and hands every caller the same one,
    // as `sync.Once` does in the Go port. Sharing is pinned by the
    // compiler (a `&mut self` parse would not fit in a `OnceLock`); what
    // is NOT pinned by the compiler, and is what this holds, is that the
    // shared engine keeps no state between parses. Failing parses are
    // interleaved with succeeding ones on purpose: a lexer or rule-stack
    // leak across calls would surface as a wrong value or a spurious
    // error here, and nowhere else in a suite that is otherwise
    // single-threaded.
    let threads: Vec<_> = (0..8)
        .map(|n| {
            std::thread::spawn(move || {
                let src = format!("{{\"n\":{n}}}\n[{n},{n}]\n{n}");
                for _ in 0..50 {
                    let out = must_parse(&src);
                    assert_eq!(out, vec![json!({"n": n}), json!([n, n]), json!(n)]);
                    assert!(parse("{\"n\":\n1}").is_err());
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("no thread panicked");
    }
}
