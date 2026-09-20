# tabnas-jsonl (Rust)

JSON Lines grammar plugin for the
[`tabnas`](https://github.com/tabnas/parser) parsing engine, crate
`tabnas_jsonl`.

[JSON Lines](https://jsonlines.org) (JSONL, also called NDJSON) is the
format log pipelines and data exports speak: one complete JSON value per
line, newline-separated, no enclosing array. A document parses to an
array of the per-line values, whatever those values are, because the
format permits any JSON value on a line:

```text
{"name":"alice","age":30}
{"name":"bob","age":25}
```

The plugin layers on the strict, standard-JSON grammar from
[`tabnas-json`](https://github.com/tabnas/json) and re-uses its whole
rule set (`val` / `map` / `list` / `pair` / `elem`) untouched. It adds no
lexer matchers. Everything it does is one lexer change (the newline stops
being an ignored token) and two rules (`jsonl`, the document, and
`record`, one line). Making the newline visible is also what refuses a
value split across lines: a newline inside a record is no longer skipped,
so pretty-printed JSON is, correctly, not JSON Lines.

This is the Rust port of the canonical TypeScript implementation in
[`../ts`](../ts); the TypeScript version is authoritative and this crate
tracks it. The Go port in [`../go`](../go) is the nearer structural
model.

## Use

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = tabnas_jsonl::parse("{\"a\":1}\n[1,2]\n\"text\"\n42")?;
    assert_eq!(records.to_string(), r#"[{"a":1},[1,2],"text",42]"#);
    Ok(())
}
```

Or build an instance and reuse it:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = tabnas_jsonl::make();
    let records = parser.parse("{\"a\":1}\n{\"b\":2}")?;
    println!("{records}");
    Ok(())
}
```

A malformed record fails the whole parse with a `JsonlError` (the
engine's `TabnasError`, re-exported) whose `row` is the line of the
offending record, so a bad line in a large file is findable:

```rust
fn main() {
    let error = tabnas_jsonl::parse("{\"a\":1}\n{\"b\":}\n{\"c\":3}").unwrap_err();
    assert_eq!(error.row, 2);
}
```

To compose the plugin by hand, install the strict-JSON grammar first and
this one second. The order is enforced: `tabnas-json` narrows the active
alternates to its own `json` tag, so applying it after this plugin would
filter these rules back out, and installing on a bare engine is refused
with an error that says so rather than failing obscurely later:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tabnas::Tabnas::new();
    tabnas_json::json(&mut parser)?;
    tabnas_jsonl::jsonl(&mut parser)?;
    Ok(())
}
```

The base has to be strict JSON, not merely a grammar that has values.
Layering on a relaxed grammar would make `{a:1}` a valid record, so the
plugin reads the three lexer options that decide record content
(`text.lex`, `comment.lex`, `string.chars`) and refuses a base that
relaxes any of them, naming the ones that are wrong.

Two boundaries: empty source is an error, inherited from `tabnas-json`
(`lex.empty` is false, matching `serde_json` on `""`), while a document
of only blank lines holds zero records and parses to `[]`.

## Install

None of `tabnas`, `tabnas-json` or this crate is published to a
registry, so all three are consumed as **sibling checkouts**, the
standard tabnas development model. Clone
`https://github.com/tabnas/parser` and `https://github.com/tabnas/json`
next to this repository and point at them:

```toml
[dependencies]
tabnas-jsonl = { path = "../jsonl/rs" }
tabnas-json = { path = "../json/rs" }
tabnas = { path = "../parser/rs" }
```

All three entries are needed. A crate's dependencies are not passed on
to its dependents, so `tabnas-jsonl` alone does not put `tabnas` or
`tabnas_json` in your extern prelude, and the composition example above
would not resolve. Only `JsonlError` is re-exported.

## Differences from the canonical TypeScript

All deliberate:

- **The IGNORE override lists the survivors.** TypeScript merges a token
  set index-wise against the default and clears the newline slot with an
  explicit `null` (`['#SP', null, '#CM']`). This engine replaces a token
  set outright, as the Go engine does, so the serialized options say
  `"IGNORE": ["#SP", "#CM"]`. Same result, different spelling.
- **The plugin is a function, not a `Plugin` value.** `jsonl(&mut
  parser)` returns `Result<(), GrammarError>`: the missing-base and
  relaxed-base refusals are returned, not thrown, and they carry the same
  named messages the other runtimes raise. Nothing re-applies the plugin
  when an instance is derived.
- **`make()` takes no options.** The TypeScript `make(opts)` and Go
  `Make(extra...)` apply extra options after the grammar exists. Apply
  them afterwards with `set_options`, which is the same ordering rule
  stated differently.
- **Record content carries `tabnas-json`'s own differences.** A record
  is parsed by the inherited strict-JSON grammar, so this port rejects
  out-of-range exponents, refuses nesting past 127 levels with the
  engine's `cancel` code, and keeps integer-like object keys in document
  order. Each is explained in the `tabnas-json` README.

## Build and test

The engine, the JSON grammar and the fixture runner are path dependencies
on sibling checkouts (`../../parser/rs`, `../../json/rs` and
`../../support/rs`), so there is nothing to fetch:

```bash
cargo test --all-targets
```

Or, from the repository root, `make test-rs`. For what CI would say,
including formatting, doctests and the lockfile check, run
`ci/rust/run.sh`.

The suite runs the shared `../test/spec/*.tsv` conformance fixtures, the
same files the TypeScript and Go suites run, through the Rust half of
`tabnas-support`, and the in-language cases those fixtures cannot state:
the API surface, the line an error reports, the layering order, a
20,000-record document parsed in one stack frame, and the shared default
parser under concurrent callers.

## License

MIT.
