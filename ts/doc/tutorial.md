# Tutorial: parsing your first JSON Lines document

This is a learning-oriented walkthrough. You start with nothing and end
with a program that reads a JSON Lines document, survives a malformed
record, and knows exactly which line was to blame. Follow it top to
bottom; every step builds on the previous one.

If you only want the API surface, read [`reference.md`](reference.md).
If you have a specific problem to solve, read [`guide.md`](guide.md). To
understand *why* it works the way it does, read
[`concepts.md`](concepts.md).

## What you are building

[JSON Lines](https://jsonlines.org) (JSONL, also called NDJSON) is the
format log pipelines, data exports, and streaming APIs use: one complete
JSON value per line, newline-separated. It is popular because a file can
be appended to, split, and processed line by line without parsing the
whole thing.

```jsonl
{"event":"login","user":"alice","ok":true}
{"event":"upload","user":"bob","bytes":10240}
{"event":"logout","user":"alice"}
```

`@tabnas/jsonl` turns that text into an array of values — one entry per
line, in order.

## Step 1 — Install

```bash
npm install @tabnas/parser @tabnas/json @tabnas/jsonl
```

Three packages, because this one is a grammar plugin rather than a
standalone parser: `@tabnas/parser` is the engine, `@tabnas/json`
supplies the strict-JSON grammar for the *content* of a record, and
`@tabnas/jsonl` adds the line structure on top. Both of the first two
are peer dependencies. Node >= 24.

## Step 2 — Parse a document

Everything you need for the common case is one function, `parse`. Give
it the whole document; get back an array of the per-line values.

```js
const { parse } = require('@tabnas/jsonl')

const rows = parse('{"user":"alice"}\n{"user":"bob"}')
rows         // => [{ user: 'alice' }, { user: 'bob' }]
rows.length  // => 2
rows[1].user // => 'bob'
```

`parse` is also the default export, so in an ES module you can write
`import parse from '@tabnas/jsonl'`.

Note the result is always an array, even for a single record. A JSONL
document is a *sequence*; one line is a sequence of one.

```js
const { parse } = require('@tabnas/jsonl')

parse('{"user":"alice"}') // => [{ user: 'alice' }]
```

## Step 3 — Any JSON value is a record

The JSON Lines format allows any JSON value on a line, not just objects.
Records do not have to share a shape, and repeated keys across lines do
not merge — each line is parsed independently.

```js
const { parse } = require('@tabnas/jsonl')

parse('1\n"two"\n[3,4]\ntrue\nnull') // => [1, 'two', [3, 4], true, null]
parse('{"a":1}\n{"a":2}')            // => [{ a: 1 }, { a: 2 }]
```

Nesting inside a record is ordinary JSON, to any depth:

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":{"b":{"c":[1,2,3]}}}') // => [{ a: { b: { c: [1, 2, 3] } } }]
```

## Step 4 — Meet the separator

Real files are untidy. The newline is the record separator, and the
parser handles the usual variations without being asked:

```js
const { parse } = require('@tabnas/jsonl')

// A trailing newline is conventional and adds no record.
parse('{"a":1}\n{"b":2}\n')   // => [{ a: 1 }, { b: 2 }]

// Blank lines between records are ignored.
parse('{"a":1}\n\n\n{"b":2}') // => [{ a: 1 }, { b: 2 }]

// A Windows line ending is one newline, not two.
parse('{"a":1}\r\n{"b":2}')   // => [{ a: 1 }, { b: 2 }]

// Spaces and tabs around a record are insignificant, as in JSON.
parse('  {"a":1}  \n\t{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

## Step 5 — See the one-record-per-line rule bite

This is the difference between JSON Lines and JSON. In a `.json` file
you may spread a value over as many lines as you like. In a `.jsonl`
file you may not: the line *is* the record boundary, which is what lets
a consumer split the file on newlines without understanding JSON.

So this fails:

```js
const { parse } = require('@tabnas/jsonl')

let failed = false
try {
  parse('{"a":1}\n{"b":\n2}')
} catch (err) {
  failed = true
}
failed // => true
```

and the identical data on one line succeeds:

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

Records must also be *separated* by that newline. Neither adjacency nor
a comma will do — a JSONL document is not a JSON array:

```js
const { parse } = require('@tabnas/jsonl')

const bad = (src) => { try { parse(src); return false } catch (e) { return true } }

bad('{"a":1}{"b":2}')  // => true
bad('{"a":1} {"b":2}') // => true
bad('{"a":1},{"b":2}') // => true
```

## Step 6 — Handle an error, and find the line

An invalid document throws a `TabnasError`. Its `lineNumber` is the line
of the offending record, which is the fact you actually need when a
large export has one bad row:

```js
const { parse, TabnasError } = require('@tabnas/jsonl')

const src = ['{"i":1}', '{"i":2}', '{"i":}', '{"i":4}'].join('\n')

let report
try {
  parse(src)
} catch (err) {
  report = err instanceof TabnasError ? 'line ' + err.lineNumber + ': ' + err.code : 'other'
}
report // => 'line 3: unexpected'
```

`TabnasError` is also exported as `JsonlError` if you prefer that name.
Alongside `code` and `lineNumber` it carries `columnNumber` and a
human-readable, source-pointing `message`.

Because a record's content is strict JSON, the things JSON rejects are
rejected here too — per record, on the line where they appear:

```js
const { parse } = require('@tabnas/jsonl')

const bad = (src) => { try { parse(src); return false } catch (e) { return true } }

bad('{a:1}')       // => true
bad("{'a':1}")     // => true
bad('{"a":1,}')    // => true
bad('{"a":01}')    // => true
bad('{"a":1} // c') // => true
```

## Step 7 — Know the empty-document boundary

An empty string throws. That is inherited from `@tabnas/json`, which
mirrors `JSON.parse('')`. A document that contains only blank lines is a
different thing: it holds zero records, and parses to an empty array.

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

If an empty file is legal input in your program, guard it at the call
site — one line:

```js
const { parse } = require('@tabnas/jsonl')

const parseDoc = (src) => ('' === src.trim() ? [] : parse(src))

parseDoc('')     // => []
parseDoc('1\n2') // => [1, 2]
```

## Step 8 — Build your own parser instance

`parse` uses one shared, lazily-built engine. When you want your own —
to hold engine options, or just to keep it explicit — call `make`:

```js
const { make } = require('@tabnas/jsonl')

const p = make()
p.parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
p.parse('{"c":3}')          // => [{ c: 3 }]
```

An instance is reusable and holds no state between parses. Building one
compiles the grammar, so build it once and parse many times.

## Step 9 — See the layering underneath

`make` is a convenience for a composition you can write yourself, and
the order carries meaning:

```js
const { Tabnas } = require('@tabnas/parser')
const { json } = require('@tabnas/json')
const { jsonl } = require('@tabnas/jsonl')

const tn = new Tabnas().use(json).use(jsonl)
tn.parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

The strict-JSON grammar goes on first; JSON Lines is a thin layer over
it. Reverse the two and the JSONL rules would be filtered straight back
out, so the plugin checks and reports it instead:

```js
const { Tabnas } = require('@tabnas/parser')
const { jsonl } = require('@tabnas/jsonl')

let message
try {
  new Tabnas().use(jsonl)
} catch (err) {
  message = err.message.includes('strict-JSON grammar must be installed first')
}
message // => true
```

[`concepts.md`](concepts.md) explains what that layer actually consists
of — it is smaller than you would guess.

## Where to go next

- [`guide.md`](guide.md) — recipes: streaming a large file, skipping bad
  records, writing JSONL back out, testing parsed records.
- [`reference.md`](reference.md) — the exact API, error codes, and
  accepted syntax.
- [`concepts.md`](concepts.md) — why one lexer change is the whole
  format.
