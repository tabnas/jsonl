# Agents Guide — shared spec fixtures

`spec/*.tsv` holds the cross-runtime conformance fixtures. Both runtimes
auto-discover and run **every** file in this directory, so a change here
affects TypeScript and Go together — edit with that in mind.

These fixtures are the whole parity mechanism for this repo. The grammar
is written twice (`ts/src/jsonl.ts` and `go/jsonl.go`) with no shared
source file to embed, so nothing but these rows stops the two copies from
drifting.

## Format

Tab-separated, one case per line, with a header row naming the columns.
Blank lines are skipped, and so are comment lines — a line starting with
`#` that contains no tab. (A data row always has at least one tab, so a
`#`-leading source still works.)

| Column | Meaning |
|---|---|
| `input` | JSONL source. Escapes `\n` `\r` `\t` `\\` are decoded. |
| `expected` | A JSON value (the parse result), or `ERROR` / `ERROR:<substring>` for inputs that must fail. |
| `opts` | Present for format compatibility with sibling repos. This plugin has **no options**; both runners fail loudly if a row sets one. |

`expected` is **not** escape-decoded — it is raw JSON, so JSON's own
escape rules apply (`"a\nb"` is a string containing a newline). To put a
literal backslash in `input`, write `\\`.

Because `input` decodes `\n`, a multi-record document is written on one
physical line: `{"a":1}\n{"b":2}`. That is deliberate — a fixture row is
one line by definition, and this format has to express documents whose
whole point is spanning several.

Results are compared after a JSON round-trip, so key order and the
`OrderedMap` / null-prototype-object representations do not affect the
comparison.

## The files

| File | What it pins |
|---|---|
| `records.tsv` | The core shape: a document is an array of per-line values; records are independent (repeated keys across lines do not merge); records need not share a shape. |
| `values.tsv` | Every JSON value type as a record — JSON Lines allows any value on a line, not only objects — plus standard string escapes. |
| `separators.tsv` | Newline handling: trailing newline, CRLF, blank lines, leading blank lines, surrounding whitespace, and the zero-record document. |
| `oneline.tsv` | **The signature rule**: a value split across lines is an error, and records must be separated by a newline rather than by adjacency, spaces or a comma. Contrast rows show the same value accepted on one line. |
| `strict.tsv` | That a record's *content* is strict JSON inherited from `@tabnas/json` — no unquoted keys, single quotes, comments, trailing commas, non-standard numbers or escapes. |

`oneline.tsv` is the one to protect. That behaviour is not written in any
grammar alternate: it falls out of `#LN` being removed from the `IGNORE`
token set. If someone puts the newline back into `IGNORE`, every other
suite still passes and only this file goes red.

## Who runs what

- TypeScript: `ts/test/parity.test.ts` — reads `../../test/spec` at
  runtime from `dist-test/`, one `describe` per file.
- Go: `go/parity_test.go` — `TestSpec` globs `../test/spec/*.tsv`.

Both discover files by directory listing: adding a `.tsv` here runs it in
both runtimes without touching either runner.

## Rules

- Prefer adding a fixture here over a one-off in-language assertion when
  a case is expressible as input → output. That is what keeps the two
  runtimes honest against each other.
- What a fixture **cannot** express, because both runners compare after a
  JSON round-trip: `bigint` / `*big.Int` values, `Infinity`, `NaN`, and
  the `-0` / `0` distinction. It also cannot express API surface, error
  line numbers, or stack behaviour — those live in `ts/test/jsonl.test.ts`
  and `go/jsonl_test.go`, mirrored case for case.
- The truly empty document (`""`) cannot be written here either: an empty
  `input` column is indistinguishable from a blank line, which the loader
  skips. That case is asserted in both in-language suites instead.
- TypeScript is canonical. If the two runtimes disagree, the TS behaviour
  is the expected value — unless Go has exposed a genuine TS defect, in
  which case fix TS first and pin the corrected behaviour here.
- A new fixture must pass in BOTH runtimes: run `go test ./...` (from
  `go/`) and `npm test` (from `ts/`) before considering it done.
