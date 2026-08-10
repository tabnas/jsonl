# Reference: `@tabnas/jsonl`

The complete public surface: exports, signatures, errors, the document
syntax accepted, and the exact configuration the plugin applies. Dry and
exhaustive. For learning see [`tutorial.md`](tutorial.md); for recipes
see [`guide.md`](guide.md); for design see [`concepts.md`](concepts.md).

- **Package:** `@tabnas/jsonl`
- **Entry point:** `dist/jsonl.js` (CommonJS), types at `dist/jsonl.d.ts`
- **Peer dependencies:** `@tabnas/parser` (the engine),
  `@tabnas/json` (the strict-JSON grammar)
- **Node:** `>=24`
- **Format:** [JSON Lines](https://jsonlines.org) (JSONL / NDJSON)

```bash
npm install @tabnas/parser @tabnas/json @tabnas/jsonl
```

## Exports

| Export | Kind | Summary |
|---|---|---|
| `parse` | function | Parse a document with the default engine. Also the default export. |
| `make` | function | Build a JSONL parser instance. |
| `jsonl` | plugin | Apply the JSONL options and rules to an engine that already has the JSON grammar. |
| `registerJsonlGrammar` | function | Register only the two document rules on an engine. |
| `VERSION` | string | Package version, always equal to `package.json`. |
| `Tabnas` | class | Re-exported engine class. |
| `TabnasError` | class | Re-exported engine error class. |
| `JsonlError` | class | Alias of `TabnasError`. |
| `default` | function | Same reference as `parse`. |

### `parse(src: string): any[]`

Parses `src` as a JSON Lines document and returns an array holding one
value per record, in document order. Uses a single, lazily-created
default engine shared across calls (each parse builds its own context, so
reuse is safe). Throws `TabnasError` on invalid input.

The return is always an array, including for a one-record document. Each
call returns a fresh array.

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
parse('{"a":1}')          // => [{ a: 1 }]
parse('\n')               // => []
```

### `make(opts?: Record<string, any>): Tabnas`

Creates a fresh engine with the `json` and `jsonl` plugins installed, in
that order:

```typescript
const tn = new Tabnas({ plugins: [json, jsonl] })
```

If `opts` is given it is applied with `tn.options(opts)` **after** the
grammar exists, so engine options layer on top of the JSONL
configuration rather than being overwritten by it. Returns a reusable
`Tabnas` instance; call `.parse(src)` on it.

```js
const { make } = require('@tabnas/jsonl')

const p = make()
p.parse('1\n2') // => [1, 2]
```

### `jsonl: Plugin`

The standard plugin form, `function jsonl(tn, _options?)`. **This plugin
has no options of its own**; the second argument is accepted and
ignored.

It performs three steps:

1. checks that the strict-JSON rule set is already installed (it looks
   for the `val` rule via `tn.rule()`), and throws otherwise;
2. applies the JSONL options (see
   [Applied options](#applied-options));
3. calls `registerJsonlGrammar(tn)`.

```js
const { Tabnas } = require('@tabnas/parser')
const { json } = require('@tabnas/json')
const { jsonl } = require('@tabnas/jsonl')

const tn = new Tabnas().use(json).use(jsonl)
tn.parse('{"a":1}\n{"b":2}') // => [{ a: 1 }, { b: 2 }]
```

Installing it on an engine without the JSON grammar throws a plain
`Error`:

```text
@tabnas/jsonl: the strict-JSON grammar must be installed first —
use `new Tabnas().use(json).use(jsonl)`, or call this package's `make()`.
```

The order is required, not conventional: `@tabnas/json` sets
`rule.include: 'json'`, which would filter this plugin's alternates back
out if it were applied afterwards.

The base must also be *strict*. A `val` rule alone is not enough — every
JSON-family grammar has one — so the plugin reads the three lexer options
that decide record content (`text.lex`, `comment.lex`, `string.chars`) and
refuses a relaxed base, naming the ones that are wrong:

```text
@tabnas/jsonl: the installed value grammar is not strict JSON (text.lex,
comment.lex, string.chars), so records would not be standard JSON. Layer
this plugin on `@tabnas/json`, not on a relaxed grammar such as
`@tabnas/jsonic`.
```

### `registerJsonlGrammar(tn: Tabnas): void`

Installs only the two document rules (`jsonl` and `record`) via
`tn.grammar({ v: 2, rule })`. It does **not** apply the JSONL options, so
the caller is responsible for `tokenSet.IGNORE`, `rule.start`, and
`rule.include`. Use it when layering a further grammar on JSONL without
re-declaring these rules.

### `VERSION: string`

The package version string, always equal to `package.json` `"version"`.
`test/version.test.ts` fails the build if the two drift, and the Go port
mirrors it as `const VERSION` in `go/jsonl.go`.

### `Tabnas` (re-export)

The engine class. Methods used with this package:

| Method | Purpose |
|---|---|
| `parse(src)` | Parse a source string. |
| `options(opts)` | Merge engine options into the instance. |
| `use(plugin, opts?)` | Install a plugin. |
| `grammar(spec)` | Install a declarative grammar spec. |
| `rule(name?, def?)` | Read or modify rules; with no argument, the whole rule map. |

### `TabnasError` / `JsonlError` (re-export + alias)

The error thrown on invalid input. `JsonlError` is the same class, not a
subclass.

| Property | Type | Meaning |
|---|---|---|
| `code` | string | Machine-readable error code (below). |
| `lineNumber` | number | 1-based source line of the offending record. |
| `columnNumber` | number | 1-based column within that line. |
| `message` | string | Human-readable message with a source extract. |

**Error codes**, inherited from the strict-JSON layer and shared with the
Go port:

| Code | When |
|---|---|
| `unexpected` | Any token no active rule alternative accepts. The catch-all: a malformed record, a record split across lines, two records with no newline between them, a comma between records, unquoted keys, trailing commas, bad numbers, empty input. |
| `unterminated_string` | A string literal with no closing quote. |
| `invalid_unicode` | A `\u` escape that is not four hex digits. |

`lineNumber` is the line the offending token sits on, so for a record
split across lines it is the line where the break occurs:

```js
const { parse } = require('@tabnas/jsonl')

const at = (src) => { try { parse(src); return 0 } catch (e) { return e.lineNumber } }

at('{"a":1}\n{"b":}\n{"c":3}')   // => 2
at('{"a":1}\n{"b":\n2}')         // => 2
at('{"a":1}\n"unterminated')     // => 2
```

## Value types

| JSON Lines | JavaScript |
|---|---|
| document | `Array` of the per-line values |
| object | plain object with **`null` prototype** (`Object.create(null)`) |
| array | `Array` |
| string | `string` (primitive) |
| number | `number` |
| `true` / `false` | `boolean` |
| `null` | `null` |

The `null` prototype comes from `@tabnas/json` and is deliberate: a
`"__proto__"` key is stored as an ordinary own property and cannot
mutate a prototype chain. Consequences:

- `Object.prototype` methods are absent — use `Object.hasOwn(obj, k)`,
  not `obj.hasOwnProperty(k)`;
- `assert.deepStrictEqual` against a plain object literal **fails** on
  the prototype difference. Compare after
  `JSON.parse(JSON.stringify(value))`.

```js
const { parse } = require('@tabnas/jsonl')

Object.getPrototypeOf(parse('{"a":1}')[0])   // => null
Array.isArray(parse('[1,2]')[0])             // => true
typeof parse('"s"')[0]                       // => 'string'
```

## Document syntax

A document is a sequence of records. A record is one complete,
standard-JSON value occupying exactly one line. The separator is the
newline.

### Records

| Input | Result |
|---|---|
| `{"a":1}` | `[{ a: 1 }]` |
| `{"a":1}\n{"b":2}` | `[{ a: 1 }, { b: 2 }]` |
| `{"a":1}\n{"a":2}` | `[{ a: 1 }, { a: 2 }]` (records never merge) |
| `1` | `[1]` |
| `{"a":1}\n[1,2]\n"text"\n42` | `[{ a: 1 }, [1, 2], 'text', 42]` |
| `{}` | `[{}]` |
| `[]` | `[[]]` |

Any JSON value is a legal record: object, array, string, number, `true`,
`false`, `null`. Records need not share a shape, and each is parsed
independently.

### Separators

| Input | Result |
|---|---|
| `{"a":1}\n` | `[{ a: 1 }]` (a trailing newline adds no record) |
| `{"a":1}\r\n{"b":2}` | `[{ a: 1 }, { b: 2 }]` (CRLF is one newline) |
| `{"a":1}\n\n\n{"b":2}` | `[{ a: 1 }, { b: 2 }]` (blank lines ignored) |
| `{"a":1}\n \n{"b":2}` | `[{ a: 1 }, { b: 2 }]` (a blank line may contain whitespace) |
| `\n{"a":1}` | `[{ a: 1 }]` (leading blank lines ignored) |
| `  {"a":1}  ` | `[{ a: 1 }]` (spaces and tabs are insignificant) |
| `\n` | `[]` (only separators: zero records) |
| `  \n  ` | `[]` (same, with whitespace) |
| `''` | **error** (empty input is rejected) |

### Rejected layouts

Every case below is an error. These are the JSON Lines rules, as opposed
to the JSON-content rules in the next section.

| Input | Why |
|---|---|
| `{"a":\n1}` | A value split across lines is not a record. |
| `[1,\n2]` | Same, in an array. |
| `{"a":1}{"b":2}` | Records must be separated by a newline. |
| `{"a":1} {"b":2}` | A space is not a record separator. |
| `{"a":1},{"b":2}` | A comma is not a separator; a document is not a JSON array. |
| `1 2` | Two values on one line. |

An escaped newline **inside** a string is data, not a separator, and
does not split the record:

```js
const { parse } = require('@tabnas/jsonl')

parse('{"a":"x\\ny"}\n{"b":2}') // => [{ a: 'x\ny' }, { b: 2 }]
```

## Record content

The content of a record is strict, standard JSON (RFC 8259 /
ECMA-404), inherited unchanged from `@tabnas/json`.

**Accepted:**

- objects `{ "key": value, ... }` with double-quoted string keys
- arrays `[ value, ... ]`
- double-quoted strings with the JSON escapes
  (`\" \\ \/ \b \f \n \r \t \uXXXX`, including surrogate pairs)
- numbers: optional `-`, integer part with no leading zeros, optional
  `.` fraction, optional `e` / `E` exponent
- `true`, `false`, `null`
- spaces and tabs between tokens

**Rejected:**

- unquoted keys (`{a:1}`)
- single-quoted or backtick strings (`{'a':1}`)
- comments (`// c`, `# c`, `/* c */`)
- trailing commas (`{"a":1,}`, `[1,2,]`)
- implicit objects and arrays (`a:1`, `1,2`)
- non-standard numbers (`01`, `+1`, `.5`, `1.`, `0x1F`, `1_000`)
- non-standard escapes (`\q`, `\x41`, `\u{41}`)
- bare words (`nope`, `undefined`)
- unterminated values (`{"a":1`, `"abc`)

## Applied options

The plugin applies exactly this options object over the strict-JSON
base:

```typescript
{
  tokenSet: { IGNORE: ['#SP', null, '#CM'] },
  rule: {
    start: 'jsonl',
    include: 'json,jsonl',
  },
}
```

| Option | Effect |
|---|---|
| `tokenSet.IGNORE` | The default ignore set is `['#SP','#LN','#CM']`; the explicit `null` clears the `#LN` slot, so newlines are no longer skipped between tokens and become matchable by the grammar. (The TypeScript engine merges token sets index-wise, hence a `null` hole rather than a shorter array; the Go engine replaces the set outright and lists the survivors.) |
| `rule.start` | The entry rule becomes `jsonl` (the document) instead of `val` (a single value). |
| `rule.include` | Widens the active alternates from `json` to `json,jsonl`, so this plugin's alternates are not filtered out by the base's own narrowing. |

Everything else — string, number, comment, key, and empty-input
handling — is inherited from `@tabnas/json` and deliberately not
restated. Notably `lex.empty: false` is why an empty source throws.

No lexer matchers are added.

## Grammar rules

The plugin registers two rules and reuses the five strict-JSON rules
unchanged:

| Rule | Origin | Role |
|---|---|---|
| `jsonl` | this plugin | The document: allocates the result array (`@array$`) and hands off to `record`. Start rule. |
| `record` | this plugin | One line: skips any spare leading separator, pushes `val`, then on `#LN` **replaces** itself to iterate, appending each value with `@push$`. |
| `val` | `@tabnas/json` | A value: map, list, or scalar token. |
| `map` | `@tabnas/json` | An object `{ ... }`. |
| `list` | `@tabnas/json` | An array `[ ... ]`. |
| `pair` | `@tabnas/json` | One `"key": value` entry. |
| `elem` | `@tabnas/json` | One array element. |

`record` iterates with `r` (replace) rather than `p` (push), so every
record is parsed in the same stack frame and rule-stack depth is
constant in the number of records; the suite parses a 20 000-record
document on that basis. Nesting *within* a record uses the stack
normally.

The value tree is built entirely by the engine's native-value
`$`-builtins (`@array$` and `@push$` here), so the grammar contains no
closures and is serializable.

## Tokens

The plugin defines no new tokens. The ones that matter to the document
rules:

| Token | Source | Role under JSONL |
|---|---|---|
| `#LN` | a run of `\r` / `\n` characters | The record separator. **Not** ignored (this plugin's one lexer change). |
| `#ZZ` | end of input | Terminates the document. |
| `#SP` | spaces and tabs | Ignored, as in JSON. |
| `#CM` | a comment | Ignored when comment lexing is enabled; off by default. |

The engine's line matcher scans a *run* of line characters into a single
`#LN` token (and counts `\n` for the row number), which is why
contiguous blank lines collapse into one separator and why CRLF counts
as one newline. A blank line that contains whitespace breaks the run
(`"\n \n"` lexes as `#LN #SP #LN`), so the extra separator is skipped by
`record`'s open alternate instead.

## Cross-runtime fixtures

Parse cases expressible as `input → JSON` live in the shared
[`test/spec/*.tsv`](../../test/spec/) fixtures, which the TypeScript and
Go suites both discover and run: `records.tsv`, `values.tsv`,
`separators.tsv`, `oneline.tsv`, `strict.tsv`. The tables in this
document are drawn from them. The in-language suite
(`ts/test/jsonl.test.ts`) covers what a fixture cannot state: the API
surface, error metadata, the layering contract, and scale.
