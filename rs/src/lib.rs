// Copyright (c) 2026 tabnas, MIT License

// The engine's error carries a code, position, hint and a formatted
// report, so it is large by design and `Result<_, TabnasError>` trips
// clippy's `result_large_err`. The engine allows the lint at its own
// crate root for the same reason, and so does tabnas-json; boxing here
// instead would make `parse` return a different shape from
// `Tabnas::parse`, `tabnas_json::parse` and the TypeScript and Go ports.
#![allow(clippy::result_large_err)]

//! A JSON Lines (JSONL, also known as NDJSON) grammar plugin for the
//! `tabnas` parsing engine.
//!
//! JSON Lines (<https://jsonlines.org>) is a text format where each line
//! is one complete, standard-JSON value, and the newline is the record
//! separator. A document parses to an array of the per-line values.
//!
//! ```text
//! {"name":"alice","age":30}
//! {"name":"bob","age":25}
//!
//! => [{"name":"alice","age":30},{"name":"bob","age":25}]
//! ```
//!
//! This plugin is a deliberately small demonstration of the engine's
//! extensible-grammar model: it adds NO lexer matchers and re-uses the
//! entire strict-JSON rule set (`val` / `map` / `list` / `pair` / `elem`)
//! from `tabnas_json` untouched. All of JSONL is expressed as
//!
//! 1. one lexer semantic change: the newline token stops being ignorable
//!    and becomes a meaningful token the grammar can match;
//! 2. two new rules: `jsonl` (the document) and `record` (one line).
//!
//! Point 1 does more work than it appears to. Once `#LN` is no longer in
//! the IGNORE token set, a newline inside a record is no longer invisible
//! to the parser, so a value SPLIT ACROSS LINES stops parsing, which is
//! exactly the JSON Lines requirement that each record occupy one line.
//! That rule is not written down anywhere below; it falls out of making
//! the separator significant.
//!
//! This is the Rust port; the TypeScript package (`ts/src/jsonl.ts`) is
//! canonical and the Go port (`go/jsonl.go`) is the nearer structural
//! model. The three are held together by the shared `test/spec/*.tsv`
//! fixtures, which every runtime discovers and runs.

use std::sync::OnceLock;

use serde_json::json;
use tabnas::{GrammarError, GrammarSpec, Plugin, PluginError, Tabnas, Value};

/// The README's Rust examples run as doctests, so a stale one fails the
/// gate rather than misleading the reader. Its `text`, `toml` and `bash`
/// fences are skipped; rustdoc runs only the `rust` ones.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}

/// This crate's version. It MUST equal `ts/package.json` "version": the
/// release orchestrator rewrites both, and `tests/version_test.rs` fails
/// the build if they drift. Mirrors `VERSION` in `ts/src/jsonl.ts` and
/// `const VERSION` in `go/jsonl.go`.
pub const VERSION: &str = "0.1.8";

/// The error a failed parse produces, re-exported so callers need not
/// depend on the engine crate directly. Its `row` is the line of the
/// offending record. Mirrors the TypeScript
/// `export { TabnasError as JsonlError }` and the Go `type JsonlError =
/// tabnas.TabnasError`.
pub use tabnas::TabnasError as JsonlError;

/// The JSONL options, applied over the strict-JSON base. Mirrors
/// `JSONL_OPTIONS` in `ts/src/jsonl.ts` and `jsonlOptions` in
/// `go/jsonl.go`.
///
/// Everything that makes a record's CONTENT strict JSON (double-quoted
/// strings, plain decimal numbers, quoted keys, no comments, no trailing
/// commas) is inherited from `tabnas_json` and deliberately not restated
/// here.
///
/// The options travel as an options-only grammar document rather than
/// through the typed `Options` struct so that they read line for line
/// like the other two runtimes' declarations, and so that the token set
/// is written by token NAME (`#SP`, `#CM`) rather than by resolved `Tin`.
fn jsonl_options() -> serde_json::Value {
    json!({
        "v": 2,
        "options": {
            // The one lexer semantic change. The default IGNORE set is
            // [#SP, #LN, #CM]: the lexer emits those tokens and the
            // parser skips them between meaningful tokens. Dropping #LN
            // hands newlines to the grammar, which is what lets `record`
            // use one as a separator.
            //
            // NOTE the shape: this engine REPLACES a token set outright,
            // as the Go engine does, so the survivors are listed. (The TS
            // engine merges index-wise against the default and clears a
            // slot with an explicit null: `['#SP', null, '#CM']`. Same
            // result, different spelling; the divergence is the
            // engine's.)
            "tokenSet": { "IGNORE": ["#SP", "#CM"] },

            "rule": {
                // Parse a whole document, not a single value.
                "start": "jsonl",

                // `tabnas_json` narrows the active alternates to its own
                // `json` tag; widen that to admit this plugin's alternates
                // too. This is why the json plugin must be installed
                // BEFORE this one: see `jsonl` below.
                "include": "json,jsonl",
            },
        },
    })
}

/// The JSONL document rules, mirroring `registerJsonlGrammar` in
/// `ts/src/jsonl.ts` and `RegisterJsonlGrammar` in `go/jsonl.go`.
///
/// The value tree is built entirely by the engine's native-value
/// `$`-builtins, so this grammar is function-free and serializable:
///
/// - `@array$`: allocate an empty array into the node (the document).
/// - `@push$`: append the just-built child value to that array.
fn jsonl_document() -> serde_json::Value {
    json!({
        // The schema version of the native-value builtins this grammar
        // binds to, matching the strict-JSON grammar it layers on.
        "v": 2,

        "rule": {
            // jsonl: the whole document, a possibly-empty sequence of
            // records. It allocates the result array and hands off to
            // `record`, which iterates. The engine's line matcher scans a
            // RUN of line characters into a single #LN token and treats
            // CRLF as one newline, so contiguous blank lines cost nothing
            // here; blank lines that contain whitespace are handled by
            // `record` (see its open alternates).
            "jsonl": {
                "open": [
                    // A document of only blank lines (or, with lex.empty,
                    // none at all) is a document of zero records.
                    { "s": "#ZZ", "a": "@array$", "g": "jsonl" },
                    { "s": "#LN #ZZ", "a": "@array$", "g": "jsonl" },

                    // Leading blank line(s), then the first record.
                    { "s": "#LN", "p": "record", "a": "@array$", "g": "jsonl" },

                    // The ordinary case: the first record starts immediately.
                    { "p": "record", "a": "@array$", "g": "jsonl" },
                ],
                "close": [
                    // `record` consumes through end-of-input, so there is
                    // nothing left to match here.
                    { "g": "jsonl" },
                ],
            },

            // record: exactly one line. Pushing `val` re-uses the whole
            // strict-JSON value grammar, so a record may be any JSON
            // value (object, array, string, number, true/false/null) as
            // the JSON Lines format allows.
            //
            // The close alternates iterate with `r` (replace) rather than
            // `p` (push), so every record is parsed in the SAME stack
            // frame: a million-line document does not grow the rule
            // stack, and each record's parent stays the `jsonl` node that
            // @push$ appends to.
            "record": {
                "open": [
                    // A separator run that ends the document: nothing more
                    // to parse, and nothing to push. Reached when the
                    // trailing blank lines were not contiguous; see the
                    // next alternate.
                    { "s": "#LN #ZZ", "g": "jsonl" },

                    // Another separator before any value. A RUN of newline
                    // characters lexes as one #LN, but a blank line
                    // containing a space does not: "\n \n" lexes as
                    // #LN #SP #LN, and #SP is still ignored, so two
                    // separators reach the grammar. Skip the extra one and
                    // try again, which makes any mix of blank and
                    // whitespace-only lines behave the same way.
                    { "s": "#LN", "r": "record", "g": "jsonl" },

                    // The line itself.
                    { "p": "val", "g": "jsonl" },
                ],
                "close": [
                    // A trailing separator at end of input: the file ends
                    // with a newline, which is conventional and adds no
                    // record.
                    { "s": "#LN #ZZ", "a": "@push$", "g": "jsonl,end" },

                    // A separator with more to come: iterate to the next
                    // record.
                    { "s": "#LN", "r": "record", "a": "@push$", "g": "jsonl,end" },

                    // End of input with no trailing newline.
                    { "s": "#ZZ", "a": "@push$", "g": "jsonl,end" },
                ],
            },
        },

        // Declared order, matching the order ts/src/jsonl.ts declares the
        // rules in. Without it the engine falls back to sorted names and
        // anything reading rule order would report this grammar
        // alphabetically.
        "ruleOrder": ["jsonl", "record"],
    })
}

/// Install the JSONL document rules on `parser`, without the options.
///
/// Exposed separately from the options (the same split `tabnas_json`'s
/// TypeScript and Go ports make) so a plugin layering on JSONL can re-use
/// the rules without re-declaring them. Nothing here checks the base:
/// that is [`jsonl`]'s job, and this is the half it calls after the
/// options are in place.
///
/// ```
/// let mut parser = tabnas::Tabnas::new();
/// tabnas_json::json(&mut parser)?;
/// tabnas_jsonl::register_jsonl_grammar(&mut parser)?;
/// assert!(parser.rule_names().iter().any(|name| name == "record"));
/// # Ok::<(), tabnas::GrammarError>(())
/// ```
pub fn register_jsonl_grammar(parser: &mut Tabnas) -> Result<(), GrammarError> {
    let spec = GrammarSpec::from_value(jsonl_document())?;
    parser.grammar(&spec)?;
    Ok(())
}

/// Install the JSON Lines grammar on an engine that ALREADY carries the
/// strict-JSON grammar, mirroring the TypeScript `jsonl` plugin and the
/// Go `Jsonl`:
///
/// ```
/// let mut parser = tabnas::Tabnas::new();
/// tabnas_json::json(&mut parser)?;
/// tabnas_jsonl::jsonl(&mut parser)?;
/// let value = parser.parse("1\n2\n3")?;
/// assert_eq!(value.to_string(), "[1,2,3]");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Order matters, and not only by convention: `tabnas_json` sets
/// `rule.include` to `json`, which would filter this plugin's alternates
/// straight back out if it were applied afterwards. Applying the JSON
/// grammar here when it is absent would silently accept the wrong order,
/// so instead the missing-grammar case is reported as a [`GrammarError`]
/// whose message names the repair. A base that is present but RELAXED is
/// reported the same way; see [`check_strict_json_base`].
pub fn jsonl(parser: &mut Tabnas) -> Result<(), GrammarError> {
    check_strict_json_base(parser)?;
    let options = GrammarSpec::from_value(jsonl_options())?;
    parser.grammar(&options)?;
    register_jsonl_grammar(parser)
}

/// The plugin as a [`Plugin`] value, for [`Tabnas::use_plugin`]. This is
/// the form the TypeScript `jsonl: Plugin` export takes, and it is what
/// lets [`Tabnas::derive`] re-apply the grammar on a child: `derive`
/// rebuilds the child from the parent's options and re-runs the plugins
/// registered through `use_plugin`, in order, so a grammar installed by
/// calling [`jsonl`] directly is not carried over.
///
/// The base check still applies, so the strict-JSON grammar has to be
/// registered first, and for `derive` it has to be registered as a plugin
/// too. `tabnas_json` exports a function, so wrap it:
///
/// ```
/// use tabnas::{Plugin, PluginError, Tabnas};
///
/// let json = Plugin::new("json", |parser, _options| {
///     tabnas_json::json(parser).map_err(|error| PluginError(error.0))
/// });
/// let mut parser = Tabnas::new();
/// parser.use_plugin(json, None)?;
/// parser.use_plugin(tabnas_jsonl::plugin(), None)?;
///
/// let child = parser.derive(|_options| {})?;
/// assert_eq!(child.parse("1\n2")?.to_string(), "[1,2]");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The plugin takes no options; the bag `use_plugin` passes is ignored,
/// as the TypeScript and Go plugin functions ignore theirs.
pub fn plugin() -> Plugin {
    Plugin::new("jsonl", |parser, _options| {
        jsonl(parser).map_err(|error| PluginError(error.0))
    })
}

/// Check that the engine carries a STRICT-JSON value grammar before
/// layering on it.
///
/// Testing for a `val` rule alone is not enough: every JSON-family grammar
/// defines one, so installing over a relaxed grammar would pass such a
/// check and then happily accept `{a:1}` as a record, a document this
/// package's own documentation says is invalid. What matters is not which
/// package installed the rules but whether a record's CONTENT is standard
/// JSON, so the check reads the three lexer options that decide exactly
/// that. A relaxed grammar fails at least one of them (jsonic lexes bare
/// text, lexes comments, and accepts `'` and backtick strings), and the
/// error names the ones that are wrong.
fn check_strict_json_base(parser: &Tabnas) -> Result<(), GrammarError> {
    if !parser.rule_names().iter().any(|name| name == "val") {
        return Err(GrammarError(
            "tabnas-jsonl: the strict-JSON grammar must be installed first: \
             call tabnas_json::json(&mut parser) before tabnas_jsonl::jsonl(&mut parser), \
             or use make()"
                .into(),
        ));
    }

    let options = parser.config();
    let mut relaxed: Vec<&str> = Vec::new();
    if options.text.lex {
        relaxed.push("text.lex");
    }
    if options.comment.lex {
        relaxed.push("comment.lex");
    }
    if options.string.chars != "\"" {
        relaxed.push("string.chars");
    }

    if !relaxed.is_empty() {
        return Err(GrammarError(format!(
            "tabnas-jsonl: the installed value grammar is not strict JSON ({}), \
             so records would not be standard JSON. Layer this plugin on \
             tabnas_json, not on a relaxed grammar such as jsonic",
            relaxed.join(", ")
        )));
    }
    Ok(())
}

/// Build a JSON Lines parser instance: a tabnas engine with the strict-JSON
/// grammar and this plugin installed, in that order.
///
/// Infallible by design, and it goes through [`jsonl`] rather than
/// duplicating the setup, so this path and installing the plugin by hand
/// cannot drift. Both documents are fixed literals and the strict-JSON
/// base is installed one line earlier, so a failure here is a bug in this
/// crate rather than anything a caller did; the Go `Make` panics for the
/// same reason.
///
/// The TypeScript `make(opts)` and Go `Make(extra...)` apply extra options
/// after the grammar exists. This `make` takes none: apply them afterwards
/// with `set_options`, which is the same ordering rule stated differently.
///
/// ```
/// let parser = tabnas_jsonl::make();
/// let value = parser.parse("{\"a\":1}\n{\"b\":2}")?;
/// assert_eq!(value.to_string(), r#"[{"a":1},{"b":2}]"#);
/// assert!(parser.parse("{\"a\":\n1}").is_err());
/// # Ok::<(), tabnas_jsonl::JsonlError>(())
/// ```
pub fn make() -> Tabnas {
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("the JSON grammar document is fixed and valid");
    jsonl(&mut parser).expect("the JSONL grammar document is fixed, and the JSON base is strict");
    parser
}

/// Parse a JSON Lines source string with the shared default parser and
/// return the array of per-line values.
///
/// The engine is built once, on first use, and reused after that. Both
/// other runtimes do the same (`sync.Once` in `go/jsonl.go`, a lazily
/// assigned module variable in `ts/src/jsonl.ts`), and reuse is safe here
/// for the same reason it is there: [`Tabnas::parse`] takes `&self` and
/// builds a fresh parse context per call, and `Tabnas` is `Send + Sync`,
/// so concurrent callers share one installed grammar instead of each
/// rebuilding it. `tests/jsonl_test.rs` pins that with a threaded test.
///
/// A malformed record fails the whole parse with a [`JsonlError`] whose
/// `row` is the line of the offending record.
///
/// ```
/// let value = tabnas_jsonl::parse("{\"a\":1}\n[1,2]\n\"text\"\n42")?;
/// assert_eq!(value.to_string(), r#"[{"a":1},[1,2],"text",42]"#);
///
/// let error = tabnas_jsonl::parse("{\"a\":1}\n{\"b\":}\n{\"c\":3}").unwrap_err();
/// assert_eq!(error.row, 2);
/// # Ok::<(), tabnas_jsonl::JsonlError>(())
/// ```
pub fn parse(src: &str) -> Result<Value, JsonlError> {
    static DEFAULT: OnceLock<Tabnas> = OnceLock::new();
    DEFAULT.get_or_init(make).parse(src)
}
