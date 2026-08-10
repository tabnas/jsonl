// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasjsonl

// jsonl_test.go — in-language tests for the Go port.
//
// Parse cases expressible as `input -> JSON` live in the shared
// test/spec/*.tsv fixtures instead, so they run in BOTH runtimes (see
// ../test/AGENTS.md). What is left here is what a fixture cannot state:
// the package's API surface, error metadata, the layering contract, and
// scale.

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"testing"

	tabnasjson "github.com/tabnas/json/go"
	tabnas "github.com/tabnas/parser/go"
)

// plain normalises through JSON so *OrderedMap and map[string]any compare
// structurally against a literal expectation.
func plain(t *testing.T, v any) any {
	t.Helper()
	raw, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var out any
	if err := json.Unmarshal(raw, &out); err != nil {
		t.Fatalf("unmarshal %s: %v", raw, err)
	}
	return out
}

func mustParse(t *testing.T, src string) any {
	t.Helper()
	got, err := Parse(src)
	if err != nil {
		t.Fatalf("parse %q: %v", src, err)
	}
	return plain(t, got)
}

func TestParseReturnsOneValuePerLine(t *testing.T) {
	got := mustParse(t, "{\"a\":1}\n{\"b\":2}")
	want := []any{
		map[string]any{"a": float64(1)},
		map[string]any{"b": float64(2)},
	}
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Errorf("got %#v, want %#v", got, want)
	}
}

func TestSingleRecordStillYieldsAList(t *testing.T) {
	got := mustParse(t, `{"a":1}`)
	list, ok := got.([]any)
	if !ok {
		t.Fatalf("expected a list, got %T", got)
	}
	if len(list) != 1 {
		t.Errorf("expected 1 record, got %d", len(list))
	}
}

func TestRepeatedParseDoesNotLeakState(t *testing.T) {
	first := mustParse(t, "{\"a\":1}\n{\"b\":2}")
	second := mustParse(t, `{"c":3}`)
	if n := len(first.([]any)); n != 2 {
		t.Errorf("first parse: expected 2 records, got %d", n)
	}
	if n := len(second.([]any)); n != 1 {
		t.Errorf("second parse: expected 1 record, got %d", n)
	}
}

func TestMakeBuildsIndependentInstances(t *testing.T) {
	one, two := Make(), Make()
	if one == two {
		t.Fatal("Make() returned the same instance twice")
	}
	for _, j := range []*tabnas.Tabnas{one, two} {
		got, err := j.Parse("1\n2")
		if err != nil {
			t.Fatalf("parse: %v", err)
		}
		if n := len(plain(t, got).([]any)); n != 2 {
			t.Errorf("expected 2 records, got %d", n)
		}
	}
}

func TestLayeringOnJson(t *testing.T) {
	j := tabnas.Make()
	if err := tabnasjson.Json(j, nil); err != nil {
		t.Fatalf("json plugin: %v", err)
	}

	// The strict-JSON rules are reused, not redefined: exactly two rules
	// are added on top of the five that arrive with the JSON grammar.
	before := append([]string(nil), j.RuleNames()...)
	if len(before) != 5 {
		t.Fatalf("expected 5 JSON rules, got %d (%v)", len(before), before)
	}

	if err := Jsonl(j, nil); err != nil {
		t.Fatalf("jsonl plugin: %v", err)
	}
	after := j.RuleNames()
	if len(after) != 7 {
		t.Fatalf("expected 7 rules after layering, got %d (%v)", len(after), after)
	}
	for _, name := range []string{"jsonl", "record", "val", "map", "list", "pair", "elem"} {
		if !hasRule(j, name) {
			t.Errorf("rule %q missing after layering", name)
		}
	}

	got, err := j.Parse("{\"a\":1}\n2")
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if n := len(plain(t, got).([]any)); n != 2 {
		t.Errorf("expected 2 records, got %d", n)
	}
}

func TestInstallOnBareEngineIsReported(t *testing.T) {
	err := Jsonl(tabnas.Make(), nil)
	if err == nil {
		t.Fatal("expected an error when the strict-JSON grammar is absent")
	}
	if !strings.Contains(err.Error(), "strict-JSON grammar must be installed first") {
		t.Errorf("expected a named error, got %q", err.Error())
	}
}

func TestRecordUsesInheritedJsonValueGrammar(t *testing.T) {
	src := `{"s":"x","n":-1.5e2,"t":true,"f":false,"z":null,"l":[1,[2]],"m":{"k":{}}}`
	got := mustParse(t, src)
	var want any
	if err := json.Unmarshal([]byte("["+src+"]"), &want); err != nil {
		t.Fatalf("bad test data: %v", err)
	}
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Errorf("got %#v, want %#v", got, want)
	}
}

func TestErrorReportsTheOffendingLine(t *testing.T) {
	_, err := Parse("{\"a\":1}\n{\"b\":}\n{\"c\":3}")
	if err == nil {
		t.Fatal("expected a parse error")
	}
	var je *JsonlError
	if !errors.As(err, &je) {
		t.Fatalf("expected a *JsonlError, got %T", err)
	}
	if je.Row != 2 {
		t.Errorf("expected the error on line 2, got line %d", je.Row)
	}
	if je.Code == "" {
		t.Error("expected an error code")
	}
}

func TestErrorLineSurvivesManyRecords(t *testing.T) {
	var b strings.Builder
	for i := 0; i < 50; i++ {
		fmt.Fprintf(&b, "{\"i\":%d}\n", i)
	}
	b.WriteString("not-json")

	_, err := Parse(b.String())
	if err == nil {
		t.Fatal("expected a parse error")
	}
	var je *JsonlError
	if !errors.As(err, &je) {
		t.Fatalf("expected a *JsonlError, got %T", err)
	}
	if je.Row != 51 {
		t.Errorf("expected the error on line 51, got line %d", je.Row)
	}
}

func TestEmptySourceIsRejectedBlankLinesAreNot(t *testing.T) {
	// Empty source is rejected by the strict-JSON base (lex.Empty false),
	// matching encoding/json on "". A document of only blank lines is a
	// different case: it holds zero records.
	if _, err := Parse(""); err == nil {
		t.Error("expected empty source to be rejected")
	}
	got, err := Parse("\n")
	if err != nil {
		t.Fatalf("blank-line document: %v", err)
	}
	if n := len(plain(t, got).([]any)); n != 0 {
		t.Errorf("expected 0 records, got %d", n)
	}
}

func TestLargeDocumentDoesNotGrowTheRuleStack(t *testing.T) {
	// `record` iterates by REPLACE, so stack depth is constant in the
	// record count. A push-based loop would fail well before this.
	const n = 20000
	var b strings.Builder
	for i := 0; i < n; i++ {
		if i > 0 {
			b.WriteByte('\n')
		}
		fmt.Fprintf(&b, "{\"i\":%d}", i)
	}
	got := mustParse(t, b.String())
	list, ok := got.([]any)
	if !ok {
		t.Fatalf("expected a list, got %T", got)
	}
	if len(list) != n {
		t.Fatalf("expected %d records, got %d", n, len(list))
	}
}

func TestVersionIsSemverShaped(t *testing.T) {
	if strings.Count(VERSION, ".") < 2 {
		t.Errorf("VERSION %q does not look like semver", VERSION)
	}
}
