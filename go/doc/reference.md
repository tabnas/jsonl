# Reference (Go)

The complete public surface of the Go `jsonl` module: exports, the error
type, the document grammar, and exactly what a record accepts. Dry and
exhaustive. For a guided introduction see the [tutorial](tutorial.md);
for task recipes see the [how-to guide](guide.md); for how it works (and
how it differs from TypeScript) see [concepts](concepts.md).

## Module

```bash
go get github.com/tabnas/jsonl/go@latest
```

```go
import (
	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)
```

| | |
|---|---|
| Module | `github.com/tabnas/jsonl/go` |
| Package | `tabnasjsonl` |
| Engine | `github.com/tabnas/parser/go` (aliased `tabnas` below) |
| Base grammar | `github.com/tabnas/json/go` (aliased `tabnasjson` below) |
| Go | 1.24+ |
| Format | [JSON Lines](https://jsonlines.org) (JSONL / NDJSON) |

## Exported symbols

| Symbol | Kind | Summary |
|---|---|---|
| `Parse` | func | Parse a document with the shared default instance. |
| `Make` | func | Build a JSON Lines parser instance. |
| `Jsonl` | plugin func | Apply the JSONL options and register the rules on an engine that already has strict JSON. |
| `RegisterJsonlGrammar` | func | Register only the `jsonl` / `record` rules. |
| `VERSION` | const string | Module version, always equal to `ts/package.json`. |
| `JsonlError` | type alias | `= tabnas.TabnasError`, the error a failed parse returns. |

### `func Parse(src string) (any, error)`

Parses a JSON Lines document and returns the slice of per-line values.
On success the concrete type is always `[]any`, one entry per record, in
source order. On failure it returns `(nil, *JsonlError)`; it never
panics.

Uses a single, lazily-created default instance (built once via
`sync.Once`), so repeated calls do not rebuild the engine and grammar.
The shared instance is safe for concurrent use: each parse builds its own
context and only reads instance state.

```go
doc, err := tabnasjsonl.Parse("{\"a\":1}\n{\"b\":2}")
// doc: []any{OrderedMap{a:1}, OrderedMap{b:2}}
```

### `func Make(extra ...tabnas.Options) *tabnas.Tabnas`

Builds a JSON Lines parser instance: a bare engine with the strict-JSON
grammar and this plugin installed, in that order. Each `extra` options
value is applied with `SetOptions` **after** the grammar exists, so rule
include/exclude filters operate on the installed alternates. Returns a
reusable, concurrency-safe `*tabnas.Tabnas`.

```go
p := tabnasjsonl.Make()
doc, err := p.Parse("1\n2\n3")
// doc: []any{float64(1), float64(2), float64(3)}
```

`Make` panics only if installing the fixed grammar spec fails — a
programmer error in this package or its dependency, not reachable from
caller input.

### `func Jsonl(j *tabnas.Tabnas, _ map[string]any) error`

The standard plugin form. Applies the JSONL option overrides
(`j.SetOptions`) and then calls `RegisterJsonlGrammar(j)`. The options
argument is ignored: this plugin has no options.

Install it on an engine that **already** carries the strict-JSON
grammar:

```go
j := tabnas.Make()
if err := j.Use(tabnasjson.Json); err != nil { /* ... */ }
if err := j.Use(tabnasjsonl.Jsonl); err != nil { /* ... */ }
```

If the rule `val` is absent, it installs nothing and returns

```
tabnasjsonl: the strict-JSON grammar must be installed first — call
tabnasjson.Json(j, nil) before Jsonl(j, nil), or use Make()
```

The base must also be *strict*. A `val` rule alone is not enough — every
JSON-family grammar has one — so the plugin reads the three lexer options
that decide record content (`text.lex`, `comment.lex`, `string.chars`) and
refuses a relaxed base, naming the ones that are wrong:

```
tabnasjsonl: the installed value grammar is not strict JSON (text.lex),
so records would not be standard JSON. Layer this plugin on
github.com/tabnas/json/go, not on a relaxed grammar such as jsonic
```

### `func RegisterJsonlGrammar(j *tabnas.Tabnas) error`

Installs only the two rules, via the engine's declarative grammar spec
(`j.Grammar(&tabnas.GrammarSpec{V: 2, Rule: ..., RuleOrder: ...})`). It
does **not** apply the option overrides, so the rules stay inert until
the newline is a token the grammar can see and `jsonl` is the start rule
— supply that yourself (see
[the guide](guide.md#install-the-rules-without-the-options)). Returns any
error from the grammar spec.

The rules are function-free: the value tree is built entirely by the
engine's native-value `$`-builtins, so the spec is serializable.

### `const VERSION string`

The module version string. It always equals the TS package's
`ts/package.json` `"version"` — `version_test.go` fails the build if the
two drift.

### `type JsonlError = tabnas.TabnasError`

A type **alias**, not a defined type: a `*JsonlError` and a
`*tabnas.TabnasError` are the same type, and either name works with
`errors.As`. Mirrors the TS re-export
`export { TabnasError as JsonlError }`.

## Errors

A failed parse returns `(nil, *JsonlError)`. Reach it with `errors.As`
and read the structured fields rather than the message:

```go
var je *tabnasjsonl.JsonlError
if errors.As(err, &je) {
	// je.Code, je.Row, je.Col
}
```

| Field | Type | Meaning |
|---|---|---|
| `Code` | `string` | Machine-readable error code (below). |
| `Detail` | `string` | Human-readable detail message. |
| `Row` | `int` | 1-based line number — the line of the offending record. |
| `Col` | `int` | 1-based column number. |
| `Pos` | `int` | 0-based character position in source. |
| `Src` | `string` | Source fragment (token text) at the error. |
| `Hint` | `string` | Additional explanatory text for the code. |

`Error()` returns a formatted, source-pointing report.

**Error codes**, inherited from the strict-JSON base and shared with the
TypeScript port:

| Code | When |
|---|---|
| `unexpected` | Any character or token no active rule alternative accepts — the catch-all. Covers a record split across lines, records not separated by a newline, and every relaxed-JSON form the strict base rejects. |
| `unterminated_string` | A string literal with no closing quote (`"abc`). |
| `invalid_unicode` | A `\u` escape that is not four hex digits (`\uZZZZ`, `\u{41}`). |

## Value types

`Parse` returns `any`; the concrete Go types are predictable:

| JSON Lines | Go |
|---|---|
| Document | `[]any` — one entry per record |
| Object | `*tabnas.OrderedMap` (`Keys []string`, `Vals map[string]any`, `Get`/`Has`/`Len`) |
| Array | `[]any` |
| String | `string` |
| Number | `float64` (integers included: `1` → `float64(1)`) |
| `true` / `false` | `bool` |
| `null` | `nil` |

`*tabnas.OrderedMap` preserves the key order of the source and
implements `json.Marshaler`. Build the instance with the engine's
`Map.Plain` option to get plain `map[string]any` objects instead
(see [the guide](guide.md#get-plain-mapstringany-records)).

## Document grammar

A document is a possibly-empty sequence of records. A record is one
complete standard-JSON value occupying exactly one line. The separator is
the newline, and only the newline.

| Source | Result |
|---|---|
| `{"a":1}` | 1 record |
| `{"a":1}\n{"b":2}` | 2 records |
| `{"a":1}\n` | 1 record — a trailing separator adds none |
| `{"a":1}\r\n{"b":2}` | 2 records — CRLF is one newline |
| `{"a":1}\n\n\n{"b":2}` | 2 records — blank lines are tolerated |
| `\n{"a":1}\n\n{"b":2}\n` | 2 records — leading and trailing blank lines too |
| `  {"a":1}  ` | 1 record — spaces and tabs around a record are insignificant |
| `\n`, `\n\n`, `  \n  `, `   ` | 0 records — a document with no record content |
| `""` (empty source) | **error** — see below |
| `{"a":\n1}` | **error** — a value split across lines is not a record |
| `{"a":1}{"b":2}` | **error** — adjacency is not a separator |
| `{"a":1} {"b":2}` | **error** — a space is not a separator |
| `{"a":1},{"b":2}` | **error** — a comma is not a separator; a document is not a JSON array |

In that table `\n`, `\r` and `\t` are the characters themselves. Inside a
JSON string the two-character escape `\n` is ordinary data, so a record
containing one stays a single record: `"a\nb"` written on one line parses
to one string.

Any JSON value may be a record — object, array, string, number, `true`,
`false`, `null` — so `1\n2\n3` is three records and
`{"a":1}\n[1,2]\n"text"` is three records of three different shapes.

**Empty source.** `""` is rejected (`Lex.Empty` is `false`, inherited
from the strict-JSON base, which matches `encoding/json` on `""`). A
source of only separators or only spaces is a different case: it holds
zero records and parses to an empty `[]any`. (Enabling `Lex.Empty` on
your own instance makes `""` return the engine's empty result, `nil` —
not a zero-record document.)

## Record content

A record's content is strict, standard JSON, inherited whole from
`github.com/tabnas/json/go` and not restated by this plugin. Accepted:
objects with double-quoted string keys, arrays, double-quoted strings
(escapes `\" \\ \/ \b \f \n \r \t` and `\uXXXX`, surrogate pairs
included), numbers (optional `-`, no-leading-zero integer, optional
fraction, optional `e`/`E` exponent), `true`, `false`, `null`, and
insignificant spaces and tabs.

Rejected, per record, exactly as `encoding/json` rejects them:

| Rejected | Example |
|---|---|
| Unquoted keys | `{a:1}` |
| Single-quoted or backtick strings | `{'a':1}`, ``{"a":`v`}`` |
| Comments | `{"a":1} // c`, `{"a":1} # c` |
| Trailing commas | `{"a":1,}`, `[1,2,]` |
| Implicit (brace-less) objects and lists | `a:1`, `1,2` |
| Non-standard numbers | `01`, `+1`, `.5`, `1.`, `0x1F`, `1_000` |
| Non-standard escapes | `"\q"`, `"\x41"` |
| Unterminated or malformed values | `{"a":1`, `{"a":}`, `"unterminated` |
| Bare words | `nope`, `{"a":undefined}` |

## Rules

`RegisterJsonlGrammar` adds two rules to the five (`val` / `map` /
`list` / `pair` / `elem`) that arrive with the strict-JSON grammar; those
five are reused untouched. Each rule is a small state machine with *open*
alternates (entering) and *close* alternates (leaving). The start rule is
`jsonl`.

### `jsonl` — the document

| Phase | Tokens | Push/Replace | Action | Meaning |
|---|---|---|---|---|
| open | `#ZZ` | — | `@array$` | End of input with no separator seen: zero records (a spaces-only source; `""` is rejected before the grammar runs). |
| open | `#LN #ZZ` | — | `@array$` | Only separators: zero records. |
| open | `#LN` | push `record` | `@array$` | Leading blank line(s), then the first record. |
| open | — | push `record` | `@array$` | The ordinary case: the first record starts immediately. |
| close | — | — | — | `record` consumes through end of input; nothing is left to match. |

### `record` — one line

| Phase | Tokens | Push/Replace | Action | Meaning |
|---|---|---|---|---|
| open | — | push `val` | — | A record is any strict-JSON value. |
| close | `#LN #ZZ` | — | `@push$` | Trailing separator at end of input. |
| close | `#LN` | replace `record` | `@push$` | Separator with more to come: iterate. |
| close | `#ZZ` | — | `@push$` | End of input with no trailing newline. |

`@array$` allocates the document array; `@push$` appends the
just-built record to it. The close alternates iterate with **replace**,
not push, so record count does not grow the rule stack — a 20 000-record
document is parsed at constant rule depth (`jsonl_test.go`).

Every alternate carries the group tag `jsonl`, which is what
`Rule.Include` (below) admits.

`RuleOrder` is declared in the grammar spec because a Go map has no
order: without it the engine falls back to sorted rule names, and
anything built on `(*Tabnas).RuleNames` would report the rules
alphabetically rather than as written.

## Tokens

| Token | Source | Role |
|---|---|---|
| `#LN` | a run of line characters (`\r`, `\n`) | The record separator. **Not** ignored under this plugin. |
| `#ZZ` | end of source | Document terminator. |
| `#SP` | spaces and tabs | Ignored. |
| `#CM` | comments | Ignored — but comment lexing is off in strict JSON, so no `#CM` is produced. |

The engine's line matcher scans a *run* of line characters into a single
`#LN` token (and treats CRLF as one newline), which is why blank lines
between records need no rule of their own.

## Options

**This plugin has no options of its own.** The `options` argument to
`Jsonl` is ignored, and `parity_test.go` fails any shared fixture row
that sets the `opts` column.

What it does set, over the strict-JSON base:

| Option | Value | Effect |
|---|---|---|
| `TokenSet["IGNORE"]` | `{"#SP", "#CM"}` | Replaces the default ignore set `{#SP, #LN, #CM}`, dropping `#LN` — the newline becomes a token the grammar can match. |
| `Rule.Start` | `"jsonl"` | Parse a whole document, not a single value. |
| `Rule.Include` | `"json,jsonl"` | Widens the json plugin's `json`-only alternate filter to admit this plugin's alternates. |

Everything else — the strict lexer, the value grammar, the error codes —
is inherited from `github.com/tabnas/json/go`.

To configure the parser, pass engine options to `Make`; they are applied
after the grammar is installed:

```go
tr := true
p := tabnasjsonl.Make(tabnas.Options{Map: &tabnas.MapOptions{Plain: &tr}})
```

## Conformance fixtures

The cross-runtime parse cases live in
[`../../test/spec/`](../../test/spec/) and are run by both runtimes
(`go/parity_test.go` globs them; `ts/test/parity.test.ts` reads the same
directory):

| File | What it pins |
|---|---|
| `records.tsv` | The document is an array of per-line values; records are independent and need not share a shape. |
| `values.tsv` | Any JSON value may be a record; escape handling per record. |
| `separators.tsv` | Trailing newline, CRLF, blank lines, leading/trailing blanks, separator-only documents, surrounding spaces. |
| `oneline.tsv` | The one-record-per-line rule: split values fail, and adjacency, spaces, or commas are not separators. |
| `strict.tsv` | Record content is strict JSON — no relaxed-JSON form was re-admitted by layering. |
