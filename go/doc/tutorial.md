# Tutorial — your first JSON Lines parse (Go)

This walks you from nothing to a working parse, through the format's one
real rule, and on to a parse error and your own parser instance. Follow
it in order; each step builds on the last. When you finish you will have
installed the module, parsed a multi-record document, read the records,
seen why a value split across lines is rejected, handled the error that
reports it, and built your own instance by layering the two plugins
yourself.

For a recipe-style index of individual tasks, see the
[how-to guide](guide.md). For exhaustive signatures and the accepted
grammar, see the [reference](reference.md). For why one lexer change is
enough to define the format — and how the Go version differs from
TypeScript — see [concepts](concepts.md).

## 1. Install

`jsonl` is a grammar plugin: it has no parser of its own. It runs on the
Tabnas engine, with the strict-JSON grammar from
`github.com/tabnas/json/go` underneath. Both are ordinary module
dependencies, so one `go get` is enough:

```bash
go get github.com/tabnas/jsonl/go@latest
```

```go
import tabnasjsonl "github.com/tabnas/jsonl/go"
```

## 2. Parse a document

`tabnasjsonl.Parse` is the one-call entry point. Give it JSON Lines
source and it returns the parsed document as `any` plus an `error`:

```go
doc, err := tabnasjsonl.Parse(`{"name":"alice","age":30}
{"name":"bob","age":25}`)
// doc: []any with two records
// err: nil
```

On success the value is always a `[]any` — one entry per line, in source
order. A single-record document is still a slice:

```go
doc, err := tabnasjsonl.Parse(`{"a":1}`)
// doc: []any of length 1
```

That is the shape of the format, not a convenience: a JSON Lines
document is a sequence of records, and a sequence of one is a sequence.

## 3. Read a record

A record that is a JSON object comes back as a `*tabnas.OrderedMap`,
which keeps the keys in the order they were written:

```go
import (
	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)

doc, _ := tabnasjsonl.Parse(`{"name":"alice","age":30}`)

rec := doc.([]any)[0].(*tabnas.OrderedMap)
name, ok := rec.Get("name") // "alice", true
keys := rec.Keys            // []string{"name", "age"}
n := rec.Len()              // 2
```

Numbers are `float64`, so `age` is `float64(30)`. If you would rather
have plain, unordered `map[string]any` records, that is one option away —
see [the guide](guide.md#get-plain-mapstringany-records).

## 4. Records need not be objects

JSON Lines allows any JSON value per line, and this parser reuses the
whole strict-JSON value grammar for record content. A document may mix
shapes freely:

```go
doc, _ := tabnasjsonl.Parse("{\"a\":1}\n[1,2]\n\"text\"\n42\ntrue\nnull")

for _, rec := range doc.([]any) {
	switch v := rec.(type) {
	case *tabnas.OrderedMap:
		_ = v // an object
	case []any:
		_ = v // an array
	case string, float64, bool:
		_ = v // a scalar
	case nil:
		// null
	}
}
```

## 5. Meet the one real rule

A record occupies exactly one line. That means the pretty-printed JSON
you would happily feed to `encoding/json` is *not* a JSON Lines record:

```go
doc, err := tabnasjsonl.Parse(`{
  "a": 1
}`)
// doc: nil
// err: non-nil — the newline after `{` ends the record, mid-value
```

Written on one line, the same value is fine:

```go
doc, err := tabnasjsonl.Parse(`{"a":1}`)
// doc: []any{...}, err: nil
```

The contrast is the whole format. Notice that the *content* of a record
did not change — only its layout. Records are separated by newlines and
nothing else: adjacency (`{"a":1}{"b":2}`), a space, or a comma between
two values are all errors, because a JSON Lines document is a sequence
of lines, not a JSON array.

## 6. Handle a parse error

A failed parse returns `(nil, error)`; it never panics. The error is a
`*tabnasjsonl.JsonlError`, reached with `errors.As`, and its `Row` is
the line of the offending record:

```go
import (
	"errors"
	"fmt"

	tabnasjsonl "github.com/tabnas/jsonl/go"
)

_, err := tabnasjsonl.Parse("{\"a\":1}\n{\"b\":}\n{\"c\":3}")

var je *tabnasjsonl.JsonlError
if errors.As(err, &je) {
	fmt.Println(je.Code) // unexpected
	fmt.Println(je.Row)  // 2 — the second line is the bad one
	fmt.Println(je.Col)  // 6
}
```

`je.Error()` is a formatted, source-pointing report you can show a user;
`Code`, `Row`, and `Col` are what you branch on. Because the record
separator is a real token, the row count stays honest however many blank
lines and records precede the failure.

## 7. Separators you do not have to think about

The conventional shapes of a `.jsonl` file all work, and for one reason
(see [concepts](concepts.md#why-blank-lines-and-crlf-are-free)): a run of
line characters is lexed as a *single* separator token.

```go
tabnasjsonl.Parse("{\"a\":1}\n")              // 1 record: a trailing newline adds none
tabnasjsonl.Parse("{\"a\":1}\n\n\n{\"b\":2}") // 2 records: blank lines are tolerated
tabnasjsonl.Parse("{\"a\":1}\r\n{\"b\":2}")   // 2 records: CRLF is one newline
tabnasjsonl.Parse("\n")                       // 0 records: only separators, no content
```

There is one boundary worth knowing now, because it surprises people:
an entirely **empty** source is rejected.

```go
_, err := tabnasjsonl.Parse("")
// err: non-nil
```

That is inherited from the strict-JSON base, which rejects `""` exactly
as `encoding/json` does. A document of blank lines is a different thing:
it is a document that holds zero records.

## 8. Build your own parser instance

`Parse` uses one shared, lazily-built instance. When you want to
configure the parser, or simply hold your own, use `Make`:

```go
p := tabnasjsonl.Make()

doc, err := p.Parse("1\n2\n3")
// doc: []any{float64(1), float64(2), float64(3)}
```

An instance is reusable and safe for concurrent use — build it once, call
`Parse` on it many times. Building the engine and grammar is the
expensive part; parsing is cheap.

## 9. See the layering

`Make` is a two-line convenience. Doing it by hand shows what this
package actually is — a small layer on the strict-JSON grammar:

```go
import (
	tabnasjson "github.com/tabnas/json/go"
	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)

j := tabnas.Make()                             // a bare engine: no grammar at all
if err := j.Use(tabnasjson.Json); err != nil { // strict JSON: val/map/list/pair/elem
	return err
}
if err := j.Use(tabnasjsonl.Jsonl); err != nil { // JSON Lines: jsonl/record
	return err
}

doc, err := j.Parse("{\"a\":1}\n2")
// doc: []any{OrderedMap{a:1}, float64(2)}
```

Swap those two lines and the second `Use` returns an error naming the
problem, rather than quietly building a parser that does not work:

```go
j := tabnas.Make()
err := j.Use(tabnasjsonl.Jsonl)
// err: tabnasjsonl: the strict-JSON grammar must be installed first — ...
```

The order is load-bearing, not stylistic;
[concepts](concepts.md#why-the-order-is-load-bearing) explains why.

## Where to go next

- [How-to guide](guide.md) — focused recipes for individual tasks.
- [Reference](reference.md) — the public API, the document grammar, and
  exactly what a record accepts.
- [Concepts](concepts.md) — how making the newline significant defines
  the format, and how the Go version differs from TypeScript.
