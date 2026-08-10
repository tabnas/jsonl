# @tabnas/jsonl

A **JSON Lines** grammar plugin for the
[tabnas](https://github.com/tabnas/parser) parsing engine.

[JSON Lines](https://jsonlines.org) (JSONL, also called NDJSON) is the
format log pipelines and data exports speak: **one complete JSON value per
line**, newline-separated, no enclosing array.

```
{"name":"alice","age":30}
{"name":"bob","age":25}
```

```js
const { parse } = require('@tabnas/jsonl')

const rows = parse('{"name":"alice"}\n{"name":"bob"}')
rows   // => [{name:'alice'},{name:'bob'}]
```

A document parses to an **array of the per-line values**, whatever those
values are — JSON Lines permits any JSON value on a line, not only objects:

```js
const { parse } = require('@tabnas/jsonl')

const values = parse('{"a":1}\n[1,2]\n"text"\n42\ntrue\nnull')
values   // => [{a:1},[1,2],'text',42,true,null]
```

## The whole plugin is one lexer change and two rules

This package exists as much to be *read* as to be used. It is a worked
example of how far a tabnas grammar can be extended without touching the
engine or writing a lexer.

It adds **no lexer matchers**, and it re-uses the entire strict-JSON rule
set — `val`, `map`, `list`, `pair`, `elem` — from
[`@tabnas/json`](https://github.com/tabnas/json), unmodified. Everything
that makes a record's *content* strict JSON is inherited, not restated.

**Step one — make the newline visible to the parser.** The engine's lexer
always emits a `#LN` token, but the parser skips whatever is listed in the
`IGNORE` token set, which by default is `#SP`, `#LN`, `#CM`. Removing
`#LN` hands newlines to the grammar:

```js ignore
tokenSet: { IGNORE: ['#SP', null, '#CM'] }
```

**Step two — add two rules.** `jsonl` is the document; `record` is one
line, and it parses that line by pushing the inherited `val` rule:

```js ignore
jsonl: {
  open: [
    { s: '#ZZ',     a: '@array$' },              // no records
    { s: '#LN #ZZ', a: '@array$' },              // only blank lines
    { s: '#LN',     p: 'record', a: '@array$' }, // leading blank line
    {               p: 'record', a: '@array$' }, // the usual case
  ],
  close: [ {} ],
},

record: {
  open: [
    { s: '#LN #ZZ' },                            // trailing separators
    { s: '#LN', r: 'record' },                   // a spare separator
    { p: 'val' },                                // one strict-JSON value
  ],
  close: [
    { s: '#LN #ZZ', a: '@push$' },               // trailing newline
    { s: '#LN', r: 'record', a: '@push$' },      // next record
    { s: '#ZZ', a: '@push$' },                   // end of input
  ],
},
```

That is the entire grammar. `@array$` and `@push$` are the engine's
native-value builtins, so it stays function-free and serializable.

### What step one buys you for free

Making the separator significant does more than let `record` match it.
Once `#LN` is no longer invisible, a newline **inside** a value stops being
skipped — so a JSON value split across lines no longer parses:

```js
const { parse } = require('@tabnas/jsonl')

// The same object, on one line and split across two.
parse('{"a":1,"b":2}')          // => [{a:1,b:2}]

let split = false
try { parse('{"a":1,\n"b":2}') } catch (e) { split = true }
split                            // => true
```

That is exactly the JSON Lines rule that each record occupies one line —
and it appears nowhere in the grammar above. It falls out of the lexer
change. Pretty-printed JSON is, correctly, not JSON Lines.

## What it accepts

Records are separated by a newline, and only by a newline. CRLF and runs of
blank lines cost the grammar nothing: the engine's line matcher scans a run
of line characters into a single token. (A blank line containing a space is
the one case that does not collapse that way, which is what the spare-
separator alternate above is for.)

```js
const { parse } = require('@tabnas/jsonl')

// A trailing newline is conventional and adds no record.
parse('{"a":1}\n')            // => [{a:1}]

// CRLF counts as one newline.
parse('{"a":1}\r\n{"b":2}')   // => [{a:1},{b:2}]

// A run of blank lines is one separator.
parse('{"a":1}\n\n\n{"b":2}') // => [{a:1},{b:2}]

// Spaces and tabs around a record are insignificant, as in JSON.
parse('  {"a":1}  ')          // => [{a:1}]

// A document of only separators holds zero records.
parse('\n')                   // => []
```

Adjacency, spaces and commas are **not** separators — a JSON Lines
document is not a JSON array:

```js
const { parse } = require('@tabnas/jsonl')

const bad = ['{"a":1} {"b":2}', '{"a":1}{"b":2}', '{"a":1},{"b":2}']
const rejected = bad.filter((src) => {
  try { parse(src); return false } catch { return true }
})
rejected.length   // => 3
```

Each record's content is strict, standard JSON, inherited from
`@tabnas/json`. Unquoted keys, single quotes, comments, trailing commas,
and non-standard numbers (`01`, `+1`, `.5`, `1.`, `0x1F`) are all rejected,
per line.

## Install

```sh
npm install @tabnas/jsonl @tabnas/json @tabnas/parser
```

`@tabnas/json` and `@tabnas/parser` are peer dependencies: the plugin
layers on the first and runs on the second.

```js
const { Tabnas } = require('@tabnas/parser')
const { json } = require('@tabnas/json')
const { jsonl } = require('@tabnas/jsonl')

const tn = new Tabnas().use(json).use(jsonl)
tn.parse('1\n2\n3')   // => [1,2,3]
```

Order matters. `@tabnas/json` restricts the active grammar to its own
alternates, so applying it *after* this plugin would filter these rules
back out. Installing on a bare engine reports that directly rather than
failing obscurely later:

```js
const { Tabnas } = require('@tabnas/parser')
const { jsonl } = require('@tabnas/jsonl')

let msg = ''
try { new Tabnas().use(jsonl) } catch (e) { msg = e.message }
msg.includes('strict-JSON grammar must be installed first')   // => true
```

The `make()` and `parse()` helpers do this for you.

## Errors

A malformed record throws a `TabnasError` reporting the line it is on, so
a bad line in a large file is findable:

```js
const { parse } = require('@tabnas/jsonl')

let line = 0
try { parse('{"a":1}\n{"b":}\n{"c":3}') } catch (e) { line = e.lineNumber }
line   // => 2
```

Parsing is fail-fast: the first bad record ends the parse. Per-record
error recovery would need the engine's multi-error support, sketched in the
parser's [LSP feasibility
report](https://github.com/tabnas/parser/blob/main/ts/doc/lsp-feasibility.md).

## Two boundaries worth knowing

**Empty source throws; a blank document does not.** `parse('')` raises an
error, inherited from `@tabnas/json` (`lex.empty: false`) and matching
`JSON.parse('')`. A document containing only blank lines is a different
case: it holds zero records and yields `[]`. If you are reading files that
may be empty, guard the call:

```js
const { parse } = require('@tabnas/jsonl')

const parseFile = (src) => ('' === src.trim() ? [] : parse(src))
parseFile('')          // => []
parseFile('{"a":1}')   // => [{a:1}]
```

**Objects have a null prototype.** The engine builds maps with no
prototype so a `__proto__` key stays ordinary data. Strict deep-equality
against a plain object literal will therefore report a difference; compare
after a JSON round-trip.

```js
const { parse } = require('@tabnas/jsonl')

Object.getPrototypeOf(parse('{"a":1}')[0])   // => null
```

## Runtimes

| Runtime | Package / module | Start here |
|---|---|---|
| **TypeScript / JavaScript** — canonical | `@tabnas/jsonl` (npm) | [`ts/README.md`](ts/README.md) |
| **Go** — port that follows the TS plugin | `github.com/tabnas/jsonl/go` | [`go/README.md`](go/README.md) |

Both runtimes are held to the same behaviour by the shared
[`test/spec/*.tsv`](test/spec/) fixtures, which each side discovers and
runs automatically.

## Documentation

- **Learning** — [TypeScript tutorial](ts/doc/tutorial.md) ·
  [Go tutorial](go/doc/tutorial.md)
- **Recipes** — [TypeScript guide](ts/doc/guide.md) ·
  [Go guide](go/doc/guide.md)
- **Reference** — [TypeScript](ts/doc/reference.md) · [Go](go/doc/reference.md)
- **Explanation** — [TypeScript concepts](ts/doc/concepts.md) ·
  [Go concepts](go/doc/concepts.md)

Working on this repo? Start with [`AGENTS.md`](AGENTS.md).

## License

MIT. Copyright (c) 2026 tabnas.
