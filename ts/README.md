# @tabnas/jsonl

A [JSON Lines](https://jsonlines.org) (JSONL, also known as NDJSON)
parser for TypeScript and JavaScript — the JSON Lines **grammar plugin**
for the [`tabnas`](https://github.com/tabnas/parser) parsing engine,
layered on the strict-JSON grammar from
[`@tabnas/json`](https://github.com/tabnas/json).

Each line of the source is one complete, standard-JSON value, and the
newline is the record separator. A document parses to an **array** of the
per-line values.

Available for TypeScript/JavaScript and [Go](../go/).

## Install

```bash
npm install @tabnas/parser @tabnas/json @tabnas/jsonl
```

`@tabnas/parser` (the engine) and `@tabnas/json` (the strict-JSON
grammar this plugin extends) are peer dependencies. Node >= 24.

## Quick example

```js
const { parse } = require('@tabnas/jsonl')

const rows = parse('{"name":"alice","age":30}\n{"name":"bob","age":25}')
rows // => [{ name: 'alice', age: 30 }, { name: 'bob', age: 25 }]
```

JSON Lines allows any JSON value on a line, not only objects, and a
one-line document is still an array of one:

```js
const { parse } = require('@tabnas/jsonl')

parse('1\n"two"\n[3]\ntrue') // => [1, 'two', [3], true]
parse('{"a":1}')             // => [{ a: 1 }]
```

`parse` is also the default export.

## One record per line

The newline is significant, so a JSON value split across lines is not a
record — it is an error. This is the JSON Lines rule that makes a file
splittable by line, and this package enforces it:

```js
const { parse } = require('@tabnas/jsonl')

let line
try {
  parse('{"a":1}\n{"b":\n2}')
} catch (err) {
  line = err.lineNumber
}
line // => 2
```

The same value written on one line parses:

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

Separator handling is forgiving in the ways a real file needs: a
trailing newline adds no record, blank lines between records are
ignored, and CRLF counts as one newline.

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n')            // => [{ a: 1 }]
parse('{"a":1}\n\n{"b":2}')   // => [{ a: 1 }, { b: 2 }]
parse('{"a":1}\r\n{"b":2}')   // => [{ a: 1 }, { b: 2 }]
```

## Use it as a plugin

The package is a grammar plugin. Install it on your own engine instance,
**after** the strict-JSON grammar:

```js
const { Tabnas } = require('@tabnas/parser')
const { json } = require('@tabnas/json')
const { jsonl } = require('@tabnas/jsonl')

const tn = new Tabnas().use(json).use(jsonl)
tn.parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

The order is not a convention. `@tabnas/json` restricts the parser to
its own `json`-tagged rule alternates, so applying it *after* this
plugin would filter the JSONL alternates back out. Installing `jsonl` on
a bare engine throws a named error rather than failing later at parse
time.

`make(opts?)` does the same composition in one call and applies any
extra engine options on top; `parse` reuses one lazily-built `make()`
instance.

## What a record may contain

A record's content is strict, standard JSON, inherited unchanged from
`@tabnas/json`: double-quoted strings and keys, plain decimal numbers,
`true` / `false` / `null`, objects and arrays. No comments, no unquoted
keys, no single quotes, no trailing commas, no `01` / `+1` / `.5` / `1.`
/ hex numbers, no `\x` escapes.

Parsed objects have a **null prototype** (`Object.create(null)`), so a
`"__proto__"` key lands as ordinary data instead of poisoning a
prototype. The visible cost: `obj.hasOwnProperty` is `undefined` (use
`Object.hasOwn`), and `assert.deepStrictEqual` against a plain object
literal fails — compare after `JSON.parse(JSON.stringify(value))`.

## Errors

An invalid document throws a `TabnasError` (also exported as
`JsonlError`) carrying `code`, `lineNumber`, and `columnNumber`. The
line number is the line of the offending record, which is the number you
want when a 200 000-line export has one bad row:

```js
const { parse, TabnasError } = require('@tabnas/jsonl')

const src = '{"a":1}\n{"b":2}\n{"c":}\n{"d":4}'
let at
try {
  parse(src)
} catch (err) {
  at = err instanceof TabnasError ? err.lineNumber : -1
}
at // => 3
```

Empty source throws, inherited from `@tabnas/json` (`lex.empty: false`,
matching `JSON.parse('')`). A document of only blank lines holds zero
records and parses to `[]`. Guard the empty case at the call site if an
empty file is legal input for you:

```js
const { parse } = require('@tabnas/jsonl')

const parseDoc = (src) => ('' === src.trim() ? [] : parse(src))

parseDoc('')        // => []
parseDoc('\n\n')    // => []
parseDoc('1\n2')    // => [1, 2]
```

## Documentation

Full [Diátaxis](https://diataxis.fr) docs:

- [`doc/tutorial.md`](doc/tutorial.md) — learn it step by step.
- [`doc/guide.md`](doc/guide.md) — task-focused recipes.
- [`doc/reference.md`](doc/reference.md) — the exact API surface.
- [`doc/concepts.md`](doc/concepts.md) — how it works and why.

The Go port has the [equivalent docs](../go/doc/).

## How it works, in two lines

The plugin adds **no lexer matchers** and reuses the whole strict-JSON
rule set (`val` / `map` / `list` / `pair` / `elem`) untouched. It does
exactly two things:

1. drops `#LN` from the engine's `IGNORE` token set, so a newline
   becomes a token the grammar can match;
2. adds two rules — `jsonl` (the document) and `record` (one line).

The one-record-per-line rule is nowhere in that grammar. It falls out of
step 1: once newlines are no longer invisible, a value that spans one
stops parsing. See [`doc/concepts.md`](doc/concepts.md).

## Develop

```bash
npm install
npm run build     # tsc --build src test
npm test          # node --test dist-test/*.test.js
```

Cross-runtime parse cases live in the shared
[`test/spec/*.tsv`](../test/spec/) fixtures, which the TypeScript and Go
suites both discover and run. See [`../AGENTS.md`](../AGENTS.md) for
layout and conventions.

## License

MIT. Copyright (c) 2026 tabnas. See [LICENSE](LICENSE).
