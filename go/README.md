# jsonl (Go)

A [JSON Lines](https://jsonlines.org) (JSONL, also known as NDJSON)
grammar plugin for the [Tabnas](https://github.com/tabnas/parser)
parsing engine (`github.com/tabnas/parser/go`).

Each line of the source is one complete, standard-JSON value, the
newline is the record separator, and a document parses to a slice of the
per-line values.

This is the Go port; the TypeScript package ([`../ts/`](../ts/)) is
canonical. Both runtimes run the shared fixtures in
[`../test/spec/`](../test/spec/) and produce the same values.

## Install

```bash
go get github.com/tabnas/jsonl/go@latest
```

```go
import tabnasjsonl "github.com/tabnas/jsonl/go"
```

The module requires `github.com/tabnas/parser/go` (the engine) and
`github.com/tabnas/json/go` (the strict-JSON grammar it layers on). Both
are ordinary module requirements, resolved by `go get`.

## One example

`tabnasjsonl.Parse` is the one-call entry point — pass source, get the
records and an `error`:

```go
package main

import (
	"fmt"

	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)

func main() {
	doc, err := tabnasjsonl.Parse(`{"name":"alice","age":30}
{"name":"bob","age":25}`)
	if err != nil {
		panic(err)
	}

	for _, rec := range doc.([]any) {
		name, _ := rec.(*tabnas.OrderedMap).Get("name")
		fmt.Println(name)
	}
	// alice
	// bob
}
```

`Parse` returns `any`, always a `[]any` on success — one entry per
record, in source order. A JSON object parses to a
`*tabnas.OrderedMap` (insertion-ordered; `Map.Plain` yields a plain
`map[string]any` instead), an array to `[]any`, and scalars to
`float64` / `string` / `bool` / `nil`.

`Parse` reuses one lazily-built instance, so repeated calls do not
rebuild the engine, and it is safe for concurrent use. To configure the
parser, build your own instance with `tabnasjsonl.Make(extra ...)`.

## One record per line

The format's defining rule is that a record occupies exactly one line, so
a value split across lines is not a record:

```go
tabnasjsonl.Parse("{\"a\":1}\n{\"b\":2}") // 2 records
tabnasjsonl.Parse("{\n  \"a\": 1\n}")     // error at 1:2 — pretty-printed JSON is not JSONL
```

Nothing in this plugin's grammar states that rule. It follows from the
newline being a token the parser can see: see
[`doc/concepts.md`](doc/concepts.md).

## How it is put together

The plugin adds no lexer matchers and reuses the whole strict-JSON rule
set (`val` / `map` / `list` / `pair` / `elem`) from
[`github.com/tabnas/json/go`](https://github.com/tabnas/json) untouched.
It does two things:

1. drops `#LN` from the `IGNORE` token set, so a newline stops being
   skipped and becomes a token the grammar can match;
2. adds two rules — `jsonl` (the document) and `record` (one line).

Install it on an engine that already carries the strict-JSON grammar.
Order matters; `Make` does it for you:

```go
import (
	tabnasjson "github.com/tabnas/json/go"
	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)

j := tabnas.Make()
j.Use(tabnasjson.Json)   // strict JSON first
j.Use(tabnasjsonl.Jsonl) // then JSON Lines

// identical to:
j = tabnasjsonl.Make()
```

`Jsonl` on a bare engine returns an error rather than installing the
JSON grammar itself, because the two orders are not equivalent — the
json plugin narrows the active rule alternates to its own `json` tag,
which would filter this plugin's alternates back out.

## Documentation

Full documentation follows the [Diátaxis](https://diataxis.fr)
framework:

- [Tutorial](doc/tutorial.md) — a guided first parse, start to finish.
- [How-to guide](doc/guide.md) — short recipes for individual tasks.
- [Reference](doc/reference.md) — the public API, the document grammar,
  and what each record accepts.
- [Concepts](doc/concepts.md) — how one lexer change produces the
  one-record-per-line rule, and how the Go version differs from
  TypeScript.

For the canonical TypeScript implementation, see
[`../ts/README.md`](../ts/README.md).

## Test

From this directory:

```bash
go build ./...
go test ./...
```

`go test` runs the in-language suite (`jsonl_test.go`) plus every shared
fixture in [`../test/spec/`](../test/spec/), which `parity_test.go`
discovers by glob. `ts/test/parity.test.ts` discovers the same files, so
adding a `.tsv` there covers both runtimes.

## License

Copyright (c) 2026 tabnas, MIT License — see [`../LICENSE`](../LICENSE).
