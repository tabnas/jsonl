# How-to guide: `@tabnas/jsonl` recipes

Task-oriented recipes for real problems. Each section is self-contained
and assumes the package is installed (see the
[tutorial](tutorial.md) for the basics). For the full API see
[`reference.md`](reference.md); for the design see
[`concepts.md`](concepts.md).

## Parse a document

`parse` takes the whole source and returns one array entry per line, in
document order.

```js
const { parse } = require('@tabnas/jsonl')

const rows = parse('{"id":1,"tags":["a"]}\n{"id":2,"tags":[]}')
rows.length   // => 2
rows[0].tags  // => ['a']
rows[1].id    // => 2
```

## Reuse one parser instance

`parse` already reuses a single lazily-built engine, so repeated calls do
not rebuild the grammar. When you want an explicit instance — to hold
engine options, or to keep the dependency visible — build one with `make`
and keep it:

```js
const { make } = require('@tabnas/jsonl')

const p = make()
p.parse('{"a":1}') // => [{ a: 1 }]
p.parse('{"b":2}') // => [{ b: 2 }]
```

Parsing creates a fresh context per call, so one instance is safe to
reuse across many documents. Building an instance compiles the grammar,
so do not construct one per parse.

## Process a large file record by record

`parse` is a whole-document call: it holds the source string and the
resulting array in memory. For a file too large for that, use the format
the way it was designed — split on newlines and parse one line at a
time. Each line is itself a complete JSONL document of one record, so
the same `parse` works, returning a one-element array:

```js
const { parse } = require('@tabnas/jsonl')

function* records(text) {
  for (const line of text.split('\n')) {
    if ('' === line.trim()) continue // blank lines carry no record
    yield parse(line)[0]
  }
}

const out = [...records('{"a":1}\n\n{"a":2}\n')]
out           // => [{ a: 1 }, { a: 2 }]
out.length    // => 2
```

With a real stream, the line splitting is `readline`'s job and the body
of the loop is the same:

```js ignore
const fs = require('node:fs')
const readline = require('node:readline')
const { parse } = require('@tabnas/jsonl')

const rl = readline.createInterface({
  input: fs.createReadStream('events.jsonl'),
  crlfDelay: Infinity,
})

for await (const line of rl) {
  if ('' === line.trim()) continue
  const record = parse(line)[0]
  // ... handle record
}
```

Parsing line by line does not rebuild anything: every call goes through
the same shared engine, which builds a fresh parse context but not a
fresh grammar.

## Report which record failed

A `TabnasError` carries the line of the offending record in
`lineNumber`, plus `columnNumber` and a `code`:

```js
const { parse, TabnasError } = require('@tabnas/jsonl')

function check(src) {
  try {
    parse(src)
    return null
  } catch (err) {
    if (err instanceof TabnasError) {
      return { line: err.lineNumber, code: err.code }
    }
    throw err
  }
}

check('{"a":1}\n{"b":2}')          // => null
check('{"a":1}\n{"b":}\n{"c":3}')  // => { line: 2, code: 'unexpected' }
check('{"a":1}\n"unterminated')    // => { line: 2, code: 'unterminated_string' }
```

The line number counts source lines, so it points at the record as the
user sees it in an editor — blank lines and all.

## Skip bad records instead of failing the document

A single malformed row aborts a whole-document `parse`. When a log or
export is expected to contain occasional junk, parse line by line and
decide per record:

```js
const { parse } = require('@tabnas/jsonl')

function parseLenient(src) {
  const ok = []
  const bad = []
  src.split('\n').forEach((line, i) => {
    if ('' === line.trim()) return
    try {
      ok.push(parse(line)[0])
    } catch (err) {
      bad.push({ line: i + 1, text: line })
    }
  })
  return { ok, bad }
}

const out = parseLenient('{"a":1}\noops\n{"b":2}')
out.ok  // => [{ a: 1 }, { b: 2 }]
out.bad // => [{ line: 2, text: 'oops' }]
```

Note the line number here comes from your own index, not from the error:
each `parse(line)` call sees a one-line document, so its `lineNumber` is
always 1.

## Handle an empty or blank document

Empty source throws, inherited from `@tabnas/json`, which matches
`JSON.parse('')`. A document of only blank lines is a different case: it
holds zero records and parses to `[]`. If an empty file is legal input,
guard the call:

```js
const { parse } = require('@tabnas/jsonl')

const parseDoc = (src) => ('' === src.trim() ? [] : parse(src))

parseDoc('')       // => []
parseDoc('   ')    // => []
parseDoc('\n\n')   // => []
parseDoc('1\n2')   // => [1, 2]
```

## Write JSON Lines back out

There is no serializer in this package, and none is needed: standard
`JSON.stringify` produces a valid record as long as you do not pass an
indent (which would insert newlines and break the one-line rule).

```js
const { parse } = require('@tabnas/jsonl')

const rows = [{ id: 1 }, { id: 2, tags: ['x'] }]
const text = rows.map((r) => JSON.stringify(r)).join('\n') + '\n'

text                 // => '{"id":1}\n{"id":2,"tags":["x"]}\n'
parse(text).length   // => 2
```

A trailing newline is conventional in JSON Lines files and adds no
record, so appending one is safe and makes the file concatenable.

## Compare parsed records in a test

Parsed objects are built with a **null prototype**, so
`assert.deepStrictEqual` against a plain object literal fails on the
prototype difference even when every key and value matches. Normalise
through JSON before comparing:

```js
const assert = require('node:assert')
const { parse } = require('@tabnas/jsonl')

const plain = (v) => JSON.parse(JSON.stringify(v))

assert.deepStrictEqual(plain(parse('{"a":1}\n{"b":2}')), [{ a: 1 }, { b: 2 }])
assert.throws(() => assert.deepStrictEqual(parse('{"a":1}'), [{ a: 1 }]))

Object.getPrototypeOf(parse('{"a":1}')[0]) // => null
```

Arrays and scalars are ordinary values and compare directly; only maps
carry the null prototype. The same normalisation is what the package's
own tests and the shared `test/spec` fixtures use.

## Read a field safely

Because records have no prototype, `Object.prototype` methods are absent
on them. Use `Object.hasOwn` (and `Object.keys`, `in`, plain property
access, which all work as usual):

```js
const { parse } = require('@tabnas/jsonl')

const [rec] = parse('{"a":1,"__proto__":{"polluted":true}}')

Object.hasOwn(rec, 'a')          // => true
rec.hasOwnProperty               // => undefined
Object.keys(rec).sort()          // => ['__proto__', 'a']
({}).polluted                    // => undefined
```

The last line is the point of the null prototype: a `__proto__` key in
untrusted input stays data.

## Install the plugin on your own engine

`make` is a convenience. When you are composing an engine yourself, add
the strict-JSON grammar first and this plugin second:

```js
const { Tabnas } = require('@tabnas/parser')
const { json } = require('@tabnas/json')
const { jsonl } = require('@tabnas/jsonl')

const tn = new Tabnas().use(json).use(jsonl)
tn.parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

`new Tabnas({ plugins: [json, jsonl] })` is equivalent. The order is
load-bearing: `@tabnas/json` narrows the parser to its own `json`-tagged
alternates, so applying it last would filter the JSONL alternates out
again. Installing `jsonl` alone throws immediately rather than producing
a confusing parse failure later:

```js
const { Tabnas } = require('@tabnas/parser')
const { jsonl } = require('@tabnas/jsonl')

let code = 'none'
try {
  new Tabnas().use(jsonl)
} catch (err) {
  code = err.message.includes('must be installed first') ? 'named' : 'other'
}
code // => 'named'
```

## Add engine options

The plugin has no options of its own. `make(opts)` applies `opts` with
`tn.options(opts)` *after* the grammar exists, so engine-level options
layer on top of the JSONL configuration. For example the `info` metadata
options from `@tabnas/json`:

```js
const { make } = require('@tabnas/jsonl')

const p = make({ info: { map: true, list: true } })
const [rec] = p.parse('{"a":[1,2]}')

Object.getOwnPropertyDescriptor(rec, '__info__').value.implicit // => false
```

Options that change what a *record* may contain belong to the
strict-JSON layer; see
[`@tabnas/json`'s guide](https://github.com/tabnas/json) for those.

## Allow comments

A variant format: JSON Lines with `#` or `//` comments in it. Comment
lexing is off in the strict-JSON base, so turn it back on through
`make`:

```js
const { make } = require('@tabnas/jsonl')

const p = make({ comment: { lex: true } })

p.parse('{"a":1} // first\n{"b":2} # second') // => [{ a: 1 }, { b: 2 }]
p.parse('// header\n{"a":1}\n// tail')        // => [{ a: 1 }]
p.parse('{"a":1}\n# between\n{"b":2}')        // => [{ a: 1 }, { b: 2 }]
```

Comments are lexed as `#CM` tokens, which stay in the engine's `IGNORE`
set, so they vanish before the document rules see them: a comment on a
record line, a comment line of its own, and a comment-only document all
behave as if the comment were not there.

```js
const { make } = require('@tabnas/jsonl')

const p = make({ comment: { lex: true } })

p.parse('// nothing but a comment') // => []
```

This changes only your instance. The package-level `parse` still rejects
comments, because JSON Lines does not have them.

## Check the version

```js
const { VERSION } = require('@tabnas/jsonl')

/^\d+\.\d+\.\d+/.test(VERSION) // => true
```

`VERSION` always equals `package.json` `"version"`;
`test/version.test.ts` fails the build if the two drift.
