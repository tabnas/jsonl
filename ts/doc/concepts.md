# Concepts: how `@tabnas/jsonl` works and why

Understanding-oriented. This explains the design: what JSON Lines
actually requires, why almost all of it is one lexer setting rather than
a grammar, and what the two added rules buy. For steps see
[`tutorial.md`](tutorial.md) and [`guide.md`](guide.md); for exact
signatures see [`reference.md`](reference.md).

## The centrepiece: the format is a lexer setting

[JSON Lines](https://jsonlines.org) has one rule that JSON does not:
**one record per line**. A value may not be spread over several lines,
because the whole point of the format is that a consumer can split a
file on newlines — with `split`, `readline`, `head`, `wc -l`, a Hadoop
input format — and get whole records without parsing anything.

The obvious way to enforce that is to write it into the grammar: rules
that track whether a newline has been seen inside a value and reject it.
This plugin does not do that. It contains no rule about newlines inside
values at all. Instead it changes one thing about the lexer:

```typescript
tokenSet: { IGNORE: ['#SP', null, '#CM'] }
```

The engine's default `IGNORE` set is `['#SP','#LN','#CM']`. Tokens in
that set are still produced by the lexer, but the parser skips over them
when looking for the next meaningful token — which is exactly why JSON
lets you put a newline anywhere whitespace is allowed. Clearing the
`#LN` slot removes that permission. A newline is now a token like `{` or
`,`: it must be matched by some rule alternative, or the parse fails.

Two consequences follow from that single line, and the second one is the
format:

1. The document rules can **use** a newline as the record separator,
   since it is now visible to them.
2. Any newline the rules do not expect is an error. Inside a value, no
   alternative accepts `#LN`, so a value split across lines stops
   parsing.

Nothing states rule 2. It is the absence of a rule.

```js
const { parse } = require('@tabnas/jsonl')

const ok = (src) => { try { parse(src); return true } catch (e) { return false } }

// The same data, laid out two ways:
ok('{"a":1,"b":2}')   // => true
ok('{"a":1,\n"b":2}') // => false
```

This is worth dwelling on because it is a shape that recurs in grammar
work on this engine: a useful amount of a format is decided by *what the
parser is allowed to ignore*, before any rule is written. The whole of
JSON Lines, on top of the JSON grammar, is one cleared slot plus two
small rules.

## The layer, in full

The plugin adds **no lexer matchers** and reuses the entire strict-JSON
rule set (`val` / `map` / `list` / `pair` / `elem`) from
[`@tabnas/json`](https://github.com/tabnas/json) untouched. Its whole
contribution:

- the `IGNORE` change above;
- `rule.start: 'jsonl'`, so a parse reads a document rather than a
  single value;
- `rule.include: 'json,jsonl'`, so its own alternates are active
  alongside the base's;
- two rules, `jsonl` and `record`.

The composition test (`ts/test/debug-model.test.ts`) checks this
structurally: the introspected model must contain exactly
`elem, jsonl, list, map, pair, record, val`, with the five JSON rules
present and unrenamed. If a future change re-implemented JSON structure
here instead of reusing it, that test goes red.

## The two rules

`jsonl` is the document. It allocates the result array and hands control
to `record`:

```text
jsonl.open:
  #ZZ                -> @array$              (no input at all: needs lex.empty)
  #LN #ZZ            -> @array$              (only blank lines)
  #LN                -> push record, @array$ (leading blank lines)
  (otherwise)        -> push record, @array$
```

`record` is one line. It pushes `val` — the whole inherited JSON value
grammar, which is why a record may be any JSON value — and then decides
what follows:

```text
record.open:
  #LN #ZZ            -> (nothing left: no record to push)
  #LN                -> replace record          (skip a spare separator)
  (otherwise)        -> push val
record.close:
  #LN #ZZ            -> @push$               (trailing newline, no new record)
  #LN                -> @push$, replace record
  #ZZ                -> @push$               (end of input, no trailing newline)
```

That is the entire format. `@array$` and `@push$` are the engine's
native-value builtins, so the grammar holds no closures and stays
serializable.

## Why `record` replaces itself instead of pushing

The close alternate that continues to the next record is `r: 'record'`
(replace), not `p: 'record'` (push). The difference is where the next
record is parsed:

- **push** would open a new rule frame per record, nested inside the
  previous one. A 1 000 000-line file would build a rule stack a million
  deep and overflow long before the end.
- **replace** reuses the current frame. The rule stack depth is constant
  in the number of records, and each record's parent stays the `jsonl`
  node that `@push$` appends to.

This matters more for JSON Lines than for most formats, because the
format exists to carry files that are long by design. The suite parses a
20 000-record document to pin it, and the graph assertion in the
composition test checks that the `record` → `record` edge really is a
close-replace, not a push.

Depth *inside* a record still uses the stack in the ordinary way: nested
objects and arrays push frames, as in any JSON parse.

## Blank lines: mostly the lexer, partly one alternate

Neither blank lines nor Windows line endings appear anywhere in the
rules, yet both work. The engine's line matcher scans a **run** of line
characters (`\r` and `\n`) into one `#LN` token, counting `\n` for the
row number. So `\n\n\n` between two records arrives as a single
separator token, and `\r\n` is one newline rather than two.

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n\n\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
parse('{"a":1}\r\n{"b":2}')   // => [{ a: 1 }, { b: 2 }]
```

The run only covers line characters, though, and a "blank" line in a
real file often is not blank: `"\n \n"` has a space in it, so it lexes
as `#LN #SP #LN`. `#SP` is still ignored, so **two** separators reach
the grammar where the close alternate expects one. That is what the
second `record` open alternate is for — on a separator with no value yet
seen, it replaces itself and looks again:

```text
{ s: '#LN', r: 'record', g: 'jsonl' }
```

Skipping in the *open* phase rather than the close phase is what makes
any mix of empty and whitespace-only lines behave identically, wherever
they appear:

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n \n{"b":2}')    // => [{ a: 1 }, { b: 2 }]
parse('{"a":1}\n \n \n{"b":2}') // => [{ a: 1 }, { b: 2 }]
parse(' \n \n {"a":1}')         // => [{ a: 1 }]
parse(' \n \n ')                // => []
```

The division of labour is worth noting: the lexer collapses what it can
into one token, and exactly one grammar alternate covers the rest. Both
halves are pinned by rows in
[`test/spec/separators.tsv`](../../test/spec/separators.tsv).

The same "the parser never sees it" logic extends to comments if you
enable them. A `#CM` token stays in the `IGNORE` set, so with
`comment: { lex: true }` a comment can sit at the end of a record line
or on a line of its own without disturbing the separator structure — see
the [guide](guide.md#allow-comments).

## Why the JSON layer goes on first

`new Tabnas().use(json).use(jsonl)` is order-sensitive, and the reason is
alternate filtering. Every grammar alternate carries group tags, and
`rule.include` selects which tags are active. `@tabnas/json` sets
`rule.include: 'json'` — a deliberate narrowing that keeps a strict JSON
parse strict. This plugin widens it to `'json,jsonl'`.

Apply them the other way round and `json`'s narrowing lands last, so the
`jsonl`-tagged alternates are filtered straight back out. The parse would
then fail in a way that says nothing about the real mistake.

The plugin could paper over this by installing `json` itself when it is
missing, but that would quietly accept the wrong order and hide the
model from the reader. Instead it checks for the `val` rule and throws a
named error:

```js
const { Tabnas } = require('@tabnas/parser')
const { jsonl } = require('@tabnas/jsonl')

let named = false
try {
  new Tabnas().use(jsonl)
} catch (err) {
  named = err.message.includes('strict-JSON grammar must be installed first')
}
named // => true
```

`make()` exists so callers who do not care about the layering never have
to think about it.

## A record's content is not this plugin's business

Everything about what a record may *contain* comes from
`@tabnas/json`: double-quoted strings and keys, plain decimal numbers,
the JSON escape set, no comments, no trailing commas, no implicit
containers. None of it is restated here, which is the point of layering.

Two inherited behaviours are worth naming because they surprise people:

**Empty input throws.** `@tabnas/json` sets `lex.empty: false` so that it
matches `JSON.parse('')`, and this plugin does not override it. A
document of only blank lines is a different case — it holds zero records
and parses to `[]` — so the boundary is between "no document" and "a
document with no records":

```js
const { parse } = require('@tabnas/jsonl')

let threw = false
try {
  parse('')
} catch (err) {
  threw = true
}
threw       // => true
parse('\n') // => []
```

Whether that is the right split is arguable: an empty file is a
perfectly ordinary JSON Lines document with zero records, and a case
could be made for `[]`. Inheriting the base's answer keeps one rule
about empty input across the whole tabnas JSON family instead of two,
and the caller-side guard is a single line (see the
[guide](guide.md#handle-an-empty-or-blank-document)).

**Objects have a null prototype.** Records are built with
`Object.create(null)`, so a `"__proto__"` key in untrusted input — which
is exactly the sort of input JSON Lines carries — is stored as ordinary
data rather than mutating a prototype chain. The cost is that parsed
objects have no `Object.prototype`, so `obj.hasOwnProperty` is
`undefined` and `assert.deepStrictEqual` against a plain literal fails
on the prototype difference. Compare after
`JSON.parse(JSON.stringify(value))`.

## Why a document is an array

`parse` returns an array even for a one-record document, and even for a
document of zero records. The alternative — returning a bare value for a
single record — would make the return type depend on the input's line
count, so every caller would need a shape check before using the result.
A sequence type for a sequence format keeps `parse(src).length`,
`for (const rec of parse(src))`, and `map` working the same way for
every document.

The array is fresh per call. The engine instance is shared by `parse`,
but a parse builds its own context and the result is not cached.

## Streaming, and what this package is not

JSON Lines is designed for streams, and the format's whole benefit is
that a consumer can process a record as soon as it has a line. This
package has no incremental or streaming API: `parse` takes a complete
source string and returns a complete array.

It does not need one. Because each line is itself a valid one-record
document, streaming is composition: split the stream into lines with
whatever you already use (`readline`, a line-delimited transform), and
call `parse` on each line, which returns a one-element array. The recipe
is in the [guide](guide.md#process-a-large-file-record-by-record). Line
splitting inside the parser would duplicate what the runtime already
does and would tie a grammar plugin to an I/O model.

## TypeScript and Go

The plugin ships in two implementations, this TypeScript one and a Go
port, and the TypeScript one is canonical. Both are held together by the
shared `test/spec/*.tsv` fixtures, which both suites discover and run,
so a behaviour change that lands in one runtime and not the other goes
red.

One difference is visible in the source and is the engine's, not the
plugin's: the TypeScript engine merges token sets **index-wise** against
the default, so clearing a slot means writing an explicit `null`
(`IGNORE: ['#SP', null, '#CM']`), while the Go engine **replaces** the
set outright and therefore lists the survivors
(`{"IGNORE": {"#SP", "#CM"}}`). Same resulting ignore set, different
spelling. See [`../../go/doc/concepts.md`](../../go/doc/concepts.md) for
the Go side.
