# Agents Guide: rs/

The Rust port of the canonical TypeScript in [`../ts`](../ts). Read
[`../AGENTS.md`](../AGENTS.md) first: it holds the cross-runtime rules
and the two structural facts about this plugin (dropping `#LN` from
IGNORE is what enforces one record per line; `record` iterates by
replace). This file only covers what is specific to this crate.

## Layout

| Path | |
|---|---|
| `src/lib.rs` | the whole port: the options document, the rules document, the base check, `jsonl`, `register_jsonl_grammar`, `make`, `parse` |
| `tests/parity_test.rs` | the shared `../test/spec/*.tsv` fixtures through `tabnas_support::Runner`, plus the `opts`-column guard and a fixture census |
| `tests/jsonl_test.rs` | the in-language cases mirrored from `go/jsonl_test.go` and `ts/test/jsonl.test.ts` |
| `tests/version_test.rs` | `Cargo.toml`, `VERSION` and `ts/package.json` must agree |
| `tests/common/mod.rs` | the JSON normaliser the in-language cases compare through |
| `README.md` | the crate front page; follows `../docs/STYLE-GUIDE.md` even though it is not in the gated list |

## Three crates by path

`Cargo.toml` takes `tabnas` (`../../parser/rs`), `tabnas-json`
(`../../json/rs`) and, as a dev-dependency, `tabnas-support`
(`../../support/rs`, feature `serde_json`). None is published. Clone all
three as siblings before running cargo, and expect `Cargo.lock` to move
whenever one of them bumps its version: `../ci/rust/run.sh` exempts
exactly those three entries when it diffs the lock, and asserts
everything else.

## The grammar travels as two documents

`jsonl_options()` is an options-only `GrammarSpec` (`tokenSet.IGNORE`,
`rule.start`, `rule.include`); `jsonl_document()` holds the two rules
and their `ruleOrder`. They are split, rather than one document like
`tabnas_json`'s, because the TypeScript port exports the rules on their
own (`registerJsonlGrammar`) and the in-language suite installs them
that way; `register_jsonl_grammar` is that half. `jsonl` applies the
options first and then the rules, after the base check.

The options are serialized rather than typed so the IGNORE set can be
written by token NAME. This engine REPLACES a token set outright, like
Go, so the document lists the survivors: `["#SP", "#CM"]`. Do not
"align" it to the TypeScript `['#SP', null, '#CM']`: `null` is not a
token name here, and the set would fail to load.

`rule.include` is evaluated at parse time, not install time, so the
order of options and rules inside `jsonl` does not matter to the
engine. The order of PLUGINS still does: `tabnas_json::json` sets
`include` back to `json`, and applying it after this plugin filters the
`jsonl`-tagged alternates out at the next parse.
`json_applied_after_jsonl_filters_the_document_rules_out` pins that.

## The base check reads options, not rule names

`check_strict_json_base` requires a `val` rule AND `text.lex == false`,
`comment.lex == false`, `string.chars == "\""`, read through
`parser.config()`. It is the same three options Go's
`checkStrictJSONBase` reads, and the error names each relaxed one in
that order. A rule-name check alone would pass a relaxed base. The
refusal is a `GrammarError` with the same wording the other two
runtimes use ("strict-JSON grammar must be installed first", "not
strict JSON (text.lex, ...)"), because the in-language tests in every
runtime match on those phrases.

`jsonl` returns `GrammarError`, the same type `tabnas_json::json`
returns, so the two plugins compose with one `?`. `JsonlError` is the
PARSE error (the engine's `TabnasError`), as in TypeScript and Go; do
not conflate the two.

## What the in-language suite cannot mirror

- **Re-application on derive needs the base registered as a plugin.**
  Go's `TestPluginSurvivesReapplication` registers the plugin through
  `Use` and derives a child. Here `plugin()` is the `use_plugin` form,
  and `the_plugin_value_survives_reapplication_on_derive` ports the case
  in full, but the engine's `derive` starts the child with NO rules and
  re-runs only registered plugins, and `tabnas_json` exports a function.
  The test wraps `tabnas_json::json` in a local `Plugin` so both grammars
  re-apply in order; `make()` installs both by function, so a child
  derived from `make()` has no grammar at all. That is the engine's
  `derive` contract, not a plugin bug.
- **Nesting 200 deep inside one record.** `tabnas_json` holds this port
  to serde_json's limit of 127 open containers, so
  `deep_nesting_inside_one_record_still_works` nests 100. The limit is
  the JSON crate's to test.

## Numbers in the in-language tests

The engine holds every number as `f64`, so `Value::to_json` yields
`1.0` where `serde_json::json!` yields `1`, and `assert_eq!` on the two
fails. `tests/common/mod.rs` normalises whole numbers to integers on
both sides (`plain`, `whole`), which is the JSON round-trip the
TypeScript and Go `plain()` helpers perform. The parity runner does not
need it: `tabnas_support::Value` compares numerically.

## The `opts` column fails by panic

The shared fixture format has an `opts` column and this plugin has no
options. The Go and TypeScript runners return an error for a row that
sets one; here the parse hook PANICS instead, because a `Failure`
returned on an `ERROR` row would count as a pass.
`an_opts_column_fails_loudly` builds such a row with `parse_spec` and
expects the panic.

## The default parser is shared

`parse` builds its engine once in a `OnceLock` and reuses it, matching
`sync.Once` in Go and the lazily assigned module variable in TS. That
is sound because `Tabnas::parse` takes `&self` and builds a fresh
context per call, and `Tabnas` is `Send + Sync`.
`the_shared_default_parser_takes_concurrent_callers` interleaves
failing parses with succeeding ones across threads. Do not "optimise"
`parse` back to `make().parse(src)`: that rebuilds both grammars per
call.

## Running it

`make test-rs` from the repository root is the fast loop.
`ci/rust/run.sh` is the full gate and is what CI would run: it adds
`cargo fmt --check`, a build, doctests, clippy at `-D warnings`, the
lockfile check and the MSRV pin (`rust-version = "1.85"`). Use
`CARGO_TERM_COLOR=never` in scripts and do not run two cargo commands
at once against the shared target directory.

## The README is doctested

`src/lib.rs` includes `README.md` as crate documentation under
`#[cfg(doctest)]`, so `cargo test --doc` compiles and runs every `rust`
fence in the README exactly as it appears on the page. Keep each fence a
complete `fn main` example (no top-level `?`, no hidden `# ` lines), and
expect `readme_examples (line N)` entries in the doctest output, one per
fence.
