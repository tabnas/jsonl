# How-to guide (Go)

Short, task-focused recipes. Each is self-contained and assumes you have
the module installed (see the [tutorial](tutorial.md) for the basics).
For the full API and the accepted grammar, follow the links into the
[reference](reference.md).

```go
import (
	tabnasjson "github.com/tabnas/json/go"
	tabnasjsonl "github.com/tabnas/jsonl/go"
	tabnas "github.com/tabnas/parser/go"
)
```

Standard-library imports (`os`, `bufio`, `errors`, `encoding/json`,
`strings`, `fmt`, `log`) are used below without restating them.

## Parse a JSON Lines string

`tabnasjsonl.Parse` takes the whole document and returns one value per
line:

```go
doc, err := tabnasjsonl.Parse("{\"a\":1}\n{\"b\":2}")
// doc: []any{OrderedMap{a:1}, OrderedMap{b:2}}
```

The no-options path reuses a single cached parser instance internally, so
repeated `tabnasjsonl.Parse(src)` calls do not rebuild the engine each
time. It is safe for concurrent use.

## Read a `.jsonl` file

There is no file API: read the bytes, parse the string.

```go
src, err := os.ReadFile("events.jsonl")
if err != nil {
	return err
}
doc, err := tabnasjsonl.Parse(string(src))
if err != nil {
	return err
}
for _, rec := range doc.([]any) {
	_ = rec
}
```

This parses the whole document into memory. For a file too large for
that, or one where a single bad line must not lose the rest, see
[parse line by line](#parse-line-by-line-tolerating-bad-records) below.

## Iterate records of mixed shape

A record is any JSON value, so a type switch is the general reader:

```go
doc, _ := tabnasjsonl.Parse("{\"a\":1}\n[1,2]\n\"text\"\n42\ntrue\nnull")

for i, rec := range doc.([]any) {
	switch v := rec.(type) {
	case *tabnas.OrderedMap:
		fmt.Printf("%d: object with %d keys\n", i, v.Len())
	case []any:
		fmt.Printf("%d: array of %d\n", i, len(v))
	case string:
		fmt.Printf("%d: string %q\n", i, v)
	case float64:
		fmt.Printf("%d: number %v\n", i, v)
	case bool:
		fmt.Printf("%d: bool %v\n", i, v)
	case nil:
		fmt.Printf("%d: null\n", i)
	}
}
```

## Decode records into your own structs

The parsed tree marshals back to JSON (`*tabnas.OrderedMap` implements
`json.Marshaler` and keeps key order), so a round trip through
`encoding/json` fills your own types:

```go
type Person struct {
	Name string `json:"name"`
	Age  int    `json:"age"`
}

doc, err := tabnasjsonl.Parse("{\"name\":\"alice\",\"age\":30}\n{\"name\":\"bob\",\"age\":25}")
if err != nil {
	return err
}

blob, err := json.Marshal(doc)
if err != nil {
	return err
}

var people []Person
if err := json.Unmarshal(blob, &people); err != nil {
	return err
}
// people: []Person{{Name: "alice", Age: 30}, {Name: "bob", Age: 25}}
```

## Get plain `map[string]any` records

Objects are `*tabnas.OrderedMap` by default, which preserves key order.
When you would rather have the unordered `map[string]any` that
`encoding/json` produces, build an instance with the engine's `Map.Plain`
option:

```go
tr := true
p := tabnasjsonl.Make(tabnas.Options{Map: &tabnas.MapOptions{Plain: &tr}})

doc, _ := p.Parse("{\"a\":1}\n{\"b\":2}")
// doc: []any{map[string]any{"a": float64(1)}, map[string]any{"b": float64(2)}}
```

`Make` applies extra options after the grammar is installed, so any
engine option can be layered this way. The package-level `Parse` is
unaffected — it keeps using its own default instance.

## Find the line that failed

A failed parse returns a `*tabnasjsonl.JsonlError` (an alias of the
engine's `*tabnas.TabnasError`). Read its fields rather than scraping the
message; `Row` is the line of the offending record:

```go
func parseFile(name string) ([]any, error) {
	src, err := os.ReadFile(name)
	if err != nil {
		return nil, err
	}
	doc, err := tabnasjsonl.Parse(string(src))
	if err != nil {
		var je *tabnasjsonl.JsonlError
		if errors.As(err, &je) {
			return nil, fmt.Errorf("%s:%d:%d: %s", name, je.Row, je.Col, je.Code)
		}
		return nil, err
	}
	return doc.([]any), nil
}
```

`je.Error()` is the formatted, source-pointing report; `Code`, `Row`,
`Col`, `Pos`, and `Src` are the structured fields. The codes come from
the strict-JSON base: `unexpected`, `unterminated_string`,
`invalid_unicode`.

## Parse line by line, tolerating bad records

`Parse` is all-or-nothing: one malformed record fails the document. When
you want to keep the good records, or the file is too large to hold in
memory, split on lines yourself and parse each line as strict JSON with
the base plugin — which is exactly what a JSON Lines record is:

```go
f, err := os.Open("events.jsonl")
if err != nil {
	return err
}
defer f.Close()

sc := bufio.NewScanner(f)
line := 0
for sc.Scan() {
	line++
	text := strings.TrimSpace(sc.Text())
	if text == "" {
		continue // a blank line is not a record
	}
	rec, err := tabnasjson.Parse(text)
	if err != nil {
		log.Printf("line %d: %v", line, err)
		continue
	}
	_ = rec
}
if err := sc.Err(); err != nil {
	return err
}
```

`bufio.Scanner` caps a line at 64 KiB by default; raise it with
`sc.Buffer(...)` for long records. Note what you give up: the splitting
is now yours, so nothing checks the document as a whole — a value spread
over two lines arrives as two separate broken records rather than one
rejected one.

## Write JSON Lines back out

`encoding/json`'s `Encoder` writes a newline after each value, which is
the JSON Lines separator:

```go
doc, _ := tabnasjsonl.Parse("{\"a\":1}\n{\"b\":[2,3]}")

enc := json.NewEncoder(os.Stdout)
for _, rec := range doc.([]any) {
	if err := enc.Encode(rec); err != nil {
		return err
	}
}
// {"a":1}
// {"b":[2,3]}
```

Do not use `json.MarshalIndent` here: a pretty-printed record spans
lines, and this parser will not read it back.

## Reuse one parser, including across goroutines

Building the engine and grammar dominates the cost; parsing is cheap. So
build one instance and keep it:

```go
p := tabnasjsonl.Make()

for _, src := range inputs {
	doc, err := p.Parse(src)
	_, _ = doc, err
}
```

An instance is safe for concurrent use — each parse builds its own
context and only reads instance state — so the same `p` can be shared by
many goroutines. With no options, `tabnasjsonl.Parse` already does this
for you behind a `sync.Once`.

## Install the plugins yourself

`Make` is a convenience over "bare engine, strict JSON, then JSON
Lines". Do it by hand when you are assembling an engine that carries
other plugins too:

```go
j := tabnas.Make()
if err := j.Use(tabnasjson.Json); err != nil {
	return err
}
if err := j.Use(tabnasjsonl.Jsonl); err != nil {
	return err
}
```

The order is required: `Jsonl` on an engine without the strict-JSON
grammar returns an error naming the problem, and re-applying `Json`
afterwards narrows the active alternates back to its own `json` tag,
which switches the JSON Lines rules off again. See
[concepts](concepts.md#why-the-order-is-load-bearing).

## Install the rules without the options

`Jsonl` does two things: applies the option overrides *and* registers the
two rules. `RegisterJsonlGrammar` does only the second, for a plugin that
wants the `jsonl` / `record` rules under its own configuration:

```go
j := tabnas.Make()
if err := tabnasjson.Json(j, nil); err != nil {
	return err
}
if err := tabnasjsonl.RegisterJsonlGrammar(j); err != nil {
	return err
}

// The rules are inert until the newline is a token the grammar can see,
// the start rule is the document, and this plugin's alternates are
// admitted by the include filter:
j.SetOptions(tabnas.Options{
	TokenSet: map[string][]string{"IGNORE": {"#SP", "#CM"}},
	Rule:     &tabnas.RuleOptions{Start: "jsonl", Include: "json,jsonl"},
})

doc, err := j.Parse("{\"a\":1}\n2")
```

Those three settings are the whole of `jsonlOptions()`; supplying them
yourself is what lets you vary them (a different start rule, a wider
include tag) while reusing the rules verbatim.

## Pin a new behaviour in both runtimes

Parse cases live in the shared fixtures at
[`../../test/spec/`](../../test/spec/), not in the Go suite. Both
runtimes discover every `.tsv` in that directory, so one row covers Go
and TypeScript:

```tsv
input	expected
{"a":1}\n{"b":2}	[{"a":1},{"b":2}]
{"a":\n1}	ERROR
```

Columns are tab-separated: `input` (with `\n`, `\r`, `\t`, `\\`
decoded), `expected` (raw JSON, or `ERROR` / `ERROR:<substring>`). The
third `opts` column exists in the shared format but this plugin has no
options, so `parity_test.go` fails a row that sets one. Reserve the Go
suite for what a fixture cannot state — API surface, error metadata,
layering, and scale.
