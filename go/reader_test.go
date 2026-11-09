// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasjsonl

// reader_test.go — a parse from a reader (ParseReader, admin ADR-30).
//
// Make declares Lex.Window, since every token of this grammar, JSON's base
// included, is the engine's own matchers', and JSON's one check reads no
// further than a number's text, so a reader is read a window at a time.
// The one lexer override, #LN dropped from IGNORE, changes which tokens the
// parser skips, not how the text is read. Every row of the shared fixtures
// parses the same from a reader, handed over a few bytes at a time, as
// from its text: the same value, or an error with the same code, position,
// row and column. The engine's hook that makes a window a few bytes is
// unexported in Go, so the window here is its own: 64 KiB of margin, block
// and read. A long file places a token of every kind across those
// boundaries, the tokens longer than the window included, and is handed
// over in pieces of several sizes. ts/test/reader.test.ts and
// rs/tests/reader_test.rs hold the other runtimes to the same, under
// windows of a few units too.

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"
	"testing"
	"unicode/utf8"

	tabnasjson "github.com/tabnas/json/go"
	tabnas "github.com/tabnas/parser/go"
	support "github.com/tabnas/support/go"
)

// readerPieces is a reader that hands its bytes over at most size at a time.
type readerPieces struct {
	bytes    []byte
	at, size int
}

func (p *readerPieces) Read(buf []byte) (int, error) {
	if p.at == len(p.bytes) {
		return 0, io.EOF
	}
	n := min(p.size, len(buf), len(p.bytes)-p.at)
	copy(buf, p.bytes[p.at:p.at+n])
	p.at += n
	return n, nil
}

func inPieces(text string, size int) io.Reader {
	return &readerPieces{bytes: []byte(text), size: size}
}

// render is a value as text, the keys of an object in their order, a
// number with its type and its sign, so that two values compare as text.
func render(v any) string {
	switch t := v.(type) {
	case *tabnas.OrderedMap:
		var b strings.Builder
		b.WriteByte('{')
		for i, k := range t.Keys {
			if 0 < i {
				b.WriteByte(',')
			}
			val, _ := t.Get(k)
			b.WriteString(strconv.Quote(k) + ":" + render(val))
		}
		b.WriteByte('}')
		return b.String()
	case []any:
		parts := make([]string, len(t))
		for i, val := range t {
			parts[i] = render(val)
		}
		return "[" + strings.Join(parts, ",") + "]"
	case string:
		return strconv.Quote(t)
	default:
		return fmt.Sprintf("%T(%v)", t, t)
	}
}

// readOutcome is what a parse gave: its value, or its error's code,
// position, row and column, with the position the diagnostic counts in
// runes beside the error's own. An error's excerpt is left out: a long
// line's may be cut where the window begins, the one thing a parse from a
// reader shows differently.
func readOutcome(t *testing.T, value any, err error) string {
	t.Helper()
	if err == nil {
		return "ok " + render(value)
	}
	var je *tabnas.TabnasError
	if !errors.As(err, &je) {
		return "error " + err.Error()
	}
	raw, merr := json.Marshal(je)
	if merr != nil {
		t.Fatalf("diagnostic: %v", merr)
	}
	var d struct {
		Pos int `json:"pos"`
	}
	if merr := json.Unmarshal(raw, &d); merr != nil {
		t.Fatalf("diagnostic: %v", merr)
	}
	return fmt.Sprintf("err %s at %d (%d) %d:%d", je.Code, je.Pos, d.Pos, je.Row, je.Col)
}

// readerRecords is a long file of count records, one a line, as
// rs/tests/reader_test.rs writes it.
func readerRecords(count int) string {
	var b strings.Builder
	for i := 0; i < count; i++ {
		fmt.Fprintf(&b, "{\"id\":%d,\"name\":\"ñ%d\",\"tags\":[\"a\",\"b\"]}\n", i, i)
	}
	return b.String()
}

// window64 is the window's margin, its block and a read: 64 KiB.
const window64 = 64 << 10

// straddlers are the tokens straddling places across the window's
// boundaries, by kind: a record of every kind of value, the runs of space
// and of lines between records, and the long ones, which are longer than
// the window's margin.
var straddlers = []struct{ kind, token string }{
	{"string", `"ab\u00e9\ud83d\ude00\n\"\\ ñ€𝄞 cd"`},
	{"number", `-1234.5678e-90`},
	{"true", `true`},
	{"false", `false`},
	{"null", `null`},
	{"an object", `{"a":[1,{"b":null}],"c":"d"}`},
	{"characters past ASCII", `"ññ€€𝄞𝄞"`},
	{"an escape", `"\ud83d\ude00"`},
	{"space", strings.Repeat(" ", 9)},
	{"lines", "\r\n\n\r\n"},
	{"long string", `"` + strings.Repeat(`ab\u00e9ñ€𝄞\n`, 5000) + `"`},
	{"long number", "1." + strings.Repeat("1234567890", 7000)},
	{"long space", strings.Repeat(" \t", 40000)},
	{"long lines", strings.Repeat("\n\r\n", 30000)},
}

// straddling is a long file in which a token of every kind spans a
// multiple of 64 KiB: each kind in turn is placed across the next
// multiple, its middle on it, with records of every kind between. A value
// is a record of its own line; a run of space or of lines follows a
// record. Where the token is characters past ASCII, the multiple falls
// inside one of them, so a read of 64 KiB splits it. Positions are bytes.
// The records between are some 1 KB each, so that the file holds about a
// thousand of them (tabnas/jsonl#57).
func straddling(t *testing.T) string {
	t.Helper()
	var b strings.Builder
	record := 0
	next := func() string {
		record++
		return fmt.Sprintf(`{"id":%d,"s":"x\ty ñ€𝄞","n":[-0.5,1e3,0],"t":true,"f":false,"z":null,"pad":"%s"}`,
			record, strings.Repeat("0123456789", 90))
	}
	b.WriteString(next())
	for _, s := range straddlers {
		half := len(s.token) / 2
		// The first multiple with room before it for a line break.
		boundary := window64 * ((b.Len() + half + 2 + window64 - 1) / window64)
		for r := next(); b.Len()+1+len(r)+half+2 < boundary; r = next() {
			b.WriteString("\n" + r)
		}
		if strings.TrimSpace(s.token) != "" {
			b.WriteByte('\n')
		}
		b.WriteString(strings.Repeat(" ", boundary-half-b.Len()))
		start := b.Len()
		b.WriteString(s.token)
		// Each token spans its multiple: begun before it, ended after it.
		if !(start < boundary && boundary < b.Len()) {
			t.Fatalf("%s: %d..%d does not span %d", s.kind, start, b.Len(), boundary)
		}
	}
	b.WriteString("\n" + next() + "\n")
	return b.String()
}

func TestReaderMakeReadsAReaderAWindowAtATime(t *testing.T) {
	if !Make().Config().Window {
		t.Fatal("Make does not declare Lex.Window")
	}
	// The plugin leaves the window to the parser that installs it.
	j := tabnas.Make()
	if err := tabnasjson.Json(j, nil); err != nil {
		t.Fatal(err)
	}
	if err := Jsonl(j, nil); err != nil {
		t.Fatal(err)
	}
	if j.Config().Window {
		t.Fatal("the Jsonl plugin declares Lex.Window")
	}
}

func TestReaderEveryRowParsesTheSameFromAReader(t *testing.T) {
	dir, err := support.FindSpecDir("")
	if err != nil {
		t.Fatal(err)
	}
	files, err := support.LoadSpecDir(dir, nil)
	if err != nil {
		t.Fatal(err)
	}
	j := Make()
	compared := 0
	for _, file := range files {
		for _, row := range file.Rows {
			input := row.UnescNamed("input")
			value, err := j.Parse(input)
			whole := readOutcome(t, value, err)
			for _, size := range []int{1, 3, 64} {
				value, err := j.ParseReader(inPieces(input, size))
				if read := readOutcome(t, value, err); read != whole {
					t.Fatalf("%s in pieces of %d:\n  read  %s\n  whole %s", row.Where(), size, read, whole)
				}
				compared++
			}
		}
	}
	if compared < 300 {
		t.Fatalf("only %d parses compared", compared)
	}
}

func TestReaderALongFileParsesTheSameFromAReader(t *testing.T) {
	// 5,000 records, some 220 KB: a Go parse of JSON Lines costs time that
	// grows with the square of the record count (tabnas/jsonl#57).
	j := Make()
	text := readerRecords(5000)
	value, err := j.Parse(text)
	whole := readOutcome(t, value, err)
	if !strings.HasPrefix(whole, "ok ") {
		t.Fatalf("%.80s", whole)
	}
	value, err = j.ParseReader(inPieces(text, 4096))
	if read := readOutcome(t, value, err); read != whole {
		t.Fatal("a long file parses differently from a reader")
	}
}

func TestReaderTokensAcrossTheWindowsBoundariesParseTheSameFromAReader(t *testing.T) {
	// As written, with an error after its last boundary, and cut inside
	// its long string.
	j := Make()
	text := straddling(t)
	if len(text) < 8*window64 {
		t.Fatalf("only %d bytes", len(text))
	}
	// The cut falls between two characters, as a text's end does.
	cut := strings.Index(text, `"ab\u00e9ñ`) + 50000
	for !utf8.RuneStart(text[cut]) {
		cut--
	}
	docs := []struct{ what, text, kind string }{
		{"as written", text, "ok "},
		{"with an error at its end", text + "x\n", "err "},
		{"cut inside its long string", text[:cut], "err "},
	}
	for _, doc := range docs {
		value, err := j.Parse(doc.text)
		whole := readOutcome(t, value, err)
		if !strings.HasPrefix(whole, doc.kind) {
			t.Fatalf("%s: %.80s", doc.what, whole)
		}
		// Pieces of 64 and 4 KiB, and of 1 MiB, which a read takes 64 KiB at
		// a time, put a read's end on every multiple; 64 KiB and 7 does not.
		for _, size := range []int{64, 4096, window64 + 7, 1 << 20} {
			value, err := j.ParseReader(inPieces(doc.text, size))
			if read := readOutcome(t, value, err); read != whole {
				t.Fatalf("%s, in pieces of %d:\n  read  %.200s\n  whole %.200s", doc.what, size, read, whole)
			}
		}
		t.Logf("reader straddling: %s: %.40s", doc.what, whole)
	}
}
