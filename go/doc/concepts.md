# Concepts (Go)

Background on how the Go JSON Lines plugin is put together, and why —
plus a section on how it differs from the canonical TypeScript version.
This is understanding-oriented reading; for steps see the
[tutorial](tutorial.md) and [how-to guide](guide.md), and for exact
signatures and the accepted grammar see the [reference](reference.md).

## A grammar plugin on a shared engine

The plugin has no parser of its own. It is a thin layer on a stack of
three pieces:

- the **Tabnas engine** (`github.com/tabnas/parser/go`) — a rule-based
  parser over a configurable, matcher-based lexer, carrying no grammar at
  all;
- the **strict-JSON grammar** (`github.com/tabnas/json/go`) — the
  `val` / `map` / `list` / `pair` / `elem` rules and the lexer settings
  that clamp them to standard JSON; and
- **this plugin** (`github.com/tabnas/jsonl/go`) — one option override
  and two rules.

It adds **no lexer matchers** and reuses the five strict-JSON rules
untouched. All of JSON Lines is expressed as:

1. one lexer semantic change — the newline token stops being ignorable;
2. two new rules — `jsonl` (the document) and `record` (one line).

The first of those does more work than it looks like it does, and is
where this document starts.

## The whole format is one lexer change

The engine's default `IGNORE` token set is `{#SP, #LN, #CM}`: the lexer
emits those tokens and the parser silently skips them between meaningful
ones. That is why standard JSON does not care about layout — a newline
inside a value is invisible to the grammar.

This plugin replaces that set with `{#SP, #CM}`:

```go
TokenSet: map[string][]string{"IGNORE": {"#SP", "#CM"}},
```

`#LN` is now an ordinary token, which the `record` rule uses as its
separator. But consider what else changed. Take a pretty-printed value:

```
{"a":
1}
```

Under strict JSON that lexes to `{`, `"a"`, `:`, `1`, `}` — the newline
never reaches the parser. Under this plugin it lexes to `{`, `"a"`, `:`,
`#LN`, `1`, `}`, and by then the parser has pushed `val` to read the
pair's value. No open alternate of `val` accepts `#LN`, so the parse
fails with `unexpected` at row 1, column 6 (the engine names the state
`val~o`).

That failure **is** the JSON Lines rule that a record occupies exactly
one line. It is not written down in the grammar at all, and no alternate
mentions it. It falls out of making the separator significant. The same
change rejects `{"a":1}{"b":2}` and `{"a":1} {"b":2}` from the other
side: `record`'s close alternates admit only `#LN` or `#ZZ`, so a second
value butted against the first fails at `record~c`. "Two values in a
row" is no longer something the grammar can describe.

[`test/spec/oneline.tsv`](../../test/spec/oneline.tsv) pins this
property in both runtimes.

## What the two rules add

`jsonl` is the document: it allocates the result array with `@array$`
and hands off to `record`. `record` is one line: it pushes `val` — the
strict-JSON value rule — and on close appends the built value with
`@push$`.

Pushing `val` is the second reason this plugin is small. A record may be
any JSON value because `val` already accepts any JSON value; the escape
handling, the number grammar, the quoted-key requirement and every
rejection in [`strict.tsv`](../../test/spec/strict.tsv) come along
unchanged. Nothing about record *content* is described here, only record
*layout*.

Both rules are function-free: the value tree is built by the engine's
native-value `$`-builtins (`@array$`, `@push$`), so the grammar spec is
plain data and stays serializable.

## Iterating by replace, not push

`record`'s close alternates iterate with `R` (replace) rather than `P`
(push):

```go
{S: "#LN", R: "record", A: "@push$", G: "jsonl"},
```

Replace reuses the current rule frame instead of stacking a new one. Two
consequences follow. Record count no longer adds stack depth: a
million-line document is read in a single `record` frame, and
`jsonl_test.go` pins this at 20 000 records — a size a push-based loop
would not survive. (What nests *inside* a record still costs depth, but
that is bounded by the record, not by the file.) And each record's parent
stays the `jsonl` node, which is what `@push$` appends to, so the
document array is built as the parse goes rather than by unwinding a deep
stack at the end.

## Why blank lines and CRLF are free

Neither blank lines nor CRLF has a rule. Both work because of how the
engine's line matcher tokenizes: it scans a **run** of line characters
(`\r` and `\n`) into a *single* `#LN` token. So `\r\n` is one separator,
and so is `\n\n\n`. A stretch of blank lines between two records arrives
at the parser as exactly one separator token, indistinguishable from a
single newline.

That collapsing is not quite total, and the gap is the reason `record`
has more than one open alternate. Anything that *interrupts* a run of
line characters splits it into two `#LN` tokens, and the second lands
where a record is expected. A blank line containing a space is the
everyday case: `"\n \n"` lexes as `#LN #SP #LN`, and `#SP` is still
ignored, so two separators reach the parser. `record` therefore opens
with

	{S: "#LN #ZZ"}          // trailing separators, nothing left to parse
	{S: "#LN", R: "record"} // a spare separator: skip it and retry

before the alternate that parses a value. With those, any mix of empty
and whitespace-only lines behaves identically, wherever it appears.

The same alternates absorb an interruption from any other ignored token.
This plugin turns comment lexing off (it inherits strict JSON), so the
case cannot arise by default; but on an instance that re-enables
comments, both `{"a":1} // note` on a record line and a comment on a
line of its own between two records parse — the spare separator its
newline creates is skipped like any other. (Verified against both
runtimes, which agree.)

## Where strictness comes from

Nothing about strictness is restated by this plugin. Double-quoted
strings, plain decimal numbers, quoted keys, no comments, no trailing
commas — all of it is the `github.com/tabnas/json/go` configuration,
which mirrors `encoding/json`. `strict.tsv` exists to prove that
*layering did not quietly re-admit anything*: relaxed-JSON forms like
`{a:1}`, `{'a':1}`, `[1,2,]`, `01`, `+1`, `.5`, `0x1F` and `"\x41"` are
still rejected, per record, after the JSON Lines rules are installed.

## Why the order is load-bearing

`Make` installs the strict-JSON plugin first and this one second. That
order is not a convention:

- `github.com/tabnas/json/go` sets `Rule.Include = "json"`, which narrows
  the active alternates to those tagged `json`.
- This plugin widens that to `Rule.Include = "json,jsonl"`, admitting its
  own alternates (every one of which is tagged `jsonl`) alongside them.

Apply them the other way round — or re-apply `Json` to an engine that
already has both — and the widening happens first, then the narrowing.
The `jsonl` alternates are filtered straight back out while `jsonl`
remains the start rule, so the parser has an entry rule with no usable
alternates and *nothing* parses: not a document, not a single JSON value.
The two rules are still installed and `RuleNames()` still lists them,
which is exactly what makes the failure hard to read from the outside.

`Jsonl` could paper over this by installing the JSON grammar itself when
it is missing. It deliberately does not: that would make the wrong order
silently *work*, hiding the filtering rule from anyone composing a third
plugin onto the same engine. Instead the missing-grammar case is
reported:

```
tabnasjsonl: the strict-JSON grammar must be installed first — call
tabnasjson.Json(j, nil) before Jsonl(j, nil), or use Make()
```

## The empty-document boundary

Two nearby cases behave differently, and the difference is deliberate:

- `""` is **rejected**. `Lex.Empty` is `false`, inherited from the
  strict-JSON base, which matches `encoding/json`: an empty string is not
  a JSON document.
- `"\n"` — or any source of only separators and spaces — parses to a
  document of **zero records**. It is content-free, not absent, and the
  `jsonl` rule has open alternates (`#ZZ` and `#LN #ZZ`) that say so.

A JSON Lines file that has been created but not yet written to is
therefore an error, while one containing a stray newline is an empty
document. That is the honest reading of the two inherited rules; if the
first is not what your caller wants, `Lex.Empty` is theirs to re-enable.

## Why one instance is reused

Building the engine and installing the grammar dominates the cost of a
parse; the parse itself is cheap. `Parse` therefore caches a single
instance behind a `sync.Once` and reuses it for every call. Reuse is safe
for concurrent callers because a parse builds its own context and only
reads instance state — the same reason a `Make()` instance can be shared
across goroutines.

## Differences from the TS version

The TypeScript implementation (`ts/src/jsonl.ts`) is canonical; this Go
module follows it. The differences below do **not** change what parses or
what it parses to — they are host-language and engine realities. Parity
on behaviour is held by the shared fixtures, not by inspection.

### The `IGNORE` override is spelled differently

Both runtimes make the same change — remove `#LN` from the ignore set —
but they write it differently, because the two **engines** combine a
`tokenSet` override with the default set differently:

| | TypeScript | Go |
|---|---|---|
| Combination rule | index-wise merge against the default `['#SP','#LN','#CM']` | outright replacement |
| Clearing a slot | an explicit `null` in that position | omit the name |
| This plugin writes | `tokenSet: { IGNORE: ['#SP', null, '#CM'] }` | `TokenSet: map[string][]string{"IGNORE": {"#SP", "#CM"}}` |

TS must pad with `null` because a shorter array would leave the default
`#CM` in place at index 2; Go simply lists the survivors. The divergence
belongs to the engine, not to these ports, and is documented in the
parser port's
[`go/doc/differences.md`](https://github.com/tabnas/parser/blob/main/go/doc/differences.md)
under `options.tokenSet`.

### Go declares `RuleOrder`

A Go map has no order, so the Go grammar spec carries
`RuleOrder: []string{"jsonl", "record"}`. Without it the engine falls
back to sorted rule names and `(*Tabnas).RuleNames` — and anything built
on it, such as a grammar diagram or a debug model — would report the
rules alphabetically rather than as written. The TS object literal
already has a declaration order, so its grammar needs no equivalent.

### API shape

| Area | TypeScript | Go |
|---|---|---|
| Parse | `parse(src)` returns `any[]`, **throws** on failure | `Parse(src)` returns `(any, error)`; assert `doc.([]any)` |
| Build an instance | `make(opts?)`, one optional options object | `Make(extra ...tabnas.Options)`, variadic and strongly typed |
| Compose by hand | `new Tabnas({ plugins: [json, jsonl] })` or `.use(json).use(jsonl)` | `j := tabnas.Make(); j.Use(tabnasjson.Json); j.Use(tabnasjsonl.Jsonl)` |
| Wrong install order | the plugin **throws** | `Jsonl` **returns** an error |
| Register rules only | `registerJsonlGrammar(tn)` returns `void` | `RegisterJsonlGrammar(j)` returns `error` |
| Default instance | lazily assigned module variable (`??=`) | `sync.Once` |
| Error type | `TabnasError`, re-exported as `JsonlError`; catch and `instanceof` | `*tabnas.TabnasError`, aliased as `JsonlError`; `errors.As` |
| Entry point | a default export as well as the named ones | named exports only |

### Error fields

The codes are identical (`unexpected`, `unterminated_string`,
`invalid_unicode`) and both runtimes report the same failure at the same
place, but the field is named differently: the line of the offending
record is **`lineNumber`** in TypeScript and **`Row`** in Go (with `Col`
and `Pos` alongside). Code that branches on the location has to spell it
per runtime.

### Value types

| Value | TypeScript | Go |
|---|---|---|
| Document | `any[]` | `[]any` |
| Object | plain object with a `null` prototype; string keys keep insertion order | `*tabnas.OrderedMap` (`Map.Plain` → `map[string]any`) |
| Array | array | `[]any` |
| Number | `number` | `float64` (integers included) |
| String | `string` | `string` |
| Boolean | `boolean` | `bool` |
| Null | `null` | `nil` |

Both runtimes preserve key order by default; Go needs a dedicated type to
do it, since a Go map has none.

## Parity is held by fixtures, not by discipline

Every parse case expressible as `input → JSON` lives in
[`test/spec/*.tsv`](../../test/spec/), which **both** runtimes discover
by directory listing — `go/parity_test.go` globs the directory and
`ts/test/parity.test.ts` reads it. Adding one row runs it in Go and
TypeScript, so the two cannot drift without one of them going red.

The in-language suites keep only what a fixture cannot state: the API
surface, error metadata (`Row`, `Code`), the layering contract (rule
counts, the wrong-order error), and scale (20 000 records). That split is
why this port can be small and still be checked.
