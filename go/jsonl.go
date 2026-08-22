// Copyright (c) 2026 tabnas, MIT License

// Package jsonl is a JSON Lines (JSONL, also known as NDJSON) grammar
// plugin for the tabnas parsing engine (github.com/tabnas/parser/go).
//
// JSON Lines (https://jsonlines.org) is a text format where each line is
// one complete, standard-JSON value, and the newline is the record
// separator. A document parses to a slice of the per-line values.
//
//	{"name":"alice","age":30}
//	{"name":"bob","age":25}
//
//	=> []any{map[...]{name:alice, age:30}, map[...]{name:bob, age:25}}
//
// This plugin is a deliberately small demonstration of the engine's
// extensible-grammar model: it adds NO lexer matchers and re-uses the
// entire strict-JSON rule set (val / map / list / pair / elem) from
// github.com/tabnas/json/go untouched. All of JSONL is expressed as
//
//  1. one lexer semantic change — the newline token stops being ignorable
//     and becomes a meaningful token the grammar can match;
//  2. two new rules — jsonl (the document) and record (one line).
//
// Point 1 does more work than it appears to. Once #LN is no longer in the
// IGNORE token set, a newline inside a record is no longer invisible to
// the parser, so a value SPLIT ACROSS LINES stops parsing — which is
// exactly the JSON Lines requirement that each record occupy one line.
// That rule is not written down anywhere below; it falls out of making the
// separator significant.
//
// This is the Go port; the TypeScript package (ts/src/jsonl.ts) is
// canonical. The two are held together by the shared test/spec/*.tsv
// fixtures, which both runtimes discover and run.
package tabnasjsonl

import (
	"fmt"
	"strings"
	"sync"

	tabnasjson "github.com/tabnas/json/go"
	tabnas "github.com/tabnas/parser/go"
)

// VERSION is this module's version. It MUST equal ts/package.json
// "version": the release orchestrator rewrites both, and
// TestVersionMatchesPackageJSON fails the build if they drift.
const VERSION = "0.1.8"

// JsonlError is the error type returned by a failed parse — an alias of
// the engine's *tabnas.TabnasError (with Code / Row / Col / Hint fields
// and a formatted Error() report). Mirrors the TS re-export
// `export { TabnasError as JsonlError }`; reach it with
// `errors.As(err, &je)` where je is a *tabnasjsonl.JsonlError.
type JsonlError = tabnas.TabnasError

// jsonlOptions returns the JSONL options, applied over the strict-JSON
// base. Mirrors JSONL_OPTIONS in the TypeScript jsonl.ts.
//
// Everything that makes a record's CONTENT strict JSON — double-quoted
// strings, plain decimal numbers, quoted keys, no comments, no trailing
// commas — is inherited from github.com/tabnas/json/go and deliberately
// not restated here.
func jsonlOptions() tabnas.Options {
	return tabnas.Options{
		// The one lexer semantic change. The default ignore set is
		// {#SP, #LN, #CM}: the lexer emits those tokens and the parser
		// skips them between meaningful tokens. Dropping #LN hands
		// newlines to the grammar, which is what lets `record` use one as
		// a separator.
		//
		// NOTE the shape: the Go engine REPLACES a token set outright, so
		// the survivors are listed. (The TS engine merges index-wise
		// against the default and clears a slot with an explicit null —
		// see ts/src/jsonl.ts. Same result, different spelling; the
		// divergence is the engine's, documented in the parser port's
		// go/doc/differences.md.)
		TokenSet: map[string][]string{"IGNORE": {"#SP", "#CM"}},

		Rule: &tabnas.RuleOptions{
			// Parse a whole document, not a single value.
			Start: "jsonl",

			// The json plugin narrows the active alternates to its own
			// `json` tag; widen that to admit this plugin's alternates
			// too. This is why the json plugin must be installed BEFORE
			// this one — see Jsonl below.
			Include: "json,jsonl",
		},
	}
}

// RegisterJsonlGrammar installs the JSONL document rules on j via the
// engine's declarative grammar spec — the same shape as the TypeScript
// registerJsonlGrammar. Exposed separately from the options (the same
// split github.com/tabnas/json/go makes) so a plugin layering on JSONL can
// re-use the rules without re-declaring them.
//
// The value tree is built entirely by the engine's native-value
// $-builtins, so this grammar is function-free and serializable:
//
//	@array$ — allocate an empty array into the node (the document).
//	@push$  — append the just-built child value to that array.
func RegisterJsonlGrammar(j *tabnas.Tabnas) error {
	rules := map[string]*tabnas.GrammarRuleSpec{
		// jsonl: the whole document — a possibly-empty sequence of
		// records. It allocates the result array and hands off to
		// `record`, which iterates. The engine's line matcher scans a RUN
		// of line characters into a single #LN token and treats CRLF as
		// one newline, so contiguous blank lines cost nothing here; blank
		// lines that contain whitespace are handled by `record` (see its
		// open alternates).
		"jsonl": {
			Open: []*tabnas.GrammarAltSpec{
				// A document of only blank lines (or, with lex.Empty,
				// none at all) is a document of zero records.
				{S: "#ZZ", A: "@array$", G: "jsonl"},
				{S: "#LN #ZZ", A: "@array$", G: "jsonl"},

				// Leading blank line(s), then the first record.
				{S: "#LN", P: "record", A: "@array$", G: "jsonl"},

				// The ordinary case: the first record starts immediately.
				{P: "record", A: "@array$", G: "jsonl"},
			},
			Close: []*tabnas.GrammarAltSpec{
				// `record` consumes through end-of-input, so there is
				// nothing left to match here.
				{G: "jsonl"},
			},
		},

		// record: exactly one line. Pushing `val` re-uses the whole
		// strict-JSON value grammar, so a record may be any JSON value —
		// object, array, string, number, true/false/null — as the JSON
		// Lines format allows.
		//
		// The close alternates iterate with R (replace) rather than P
		// (push), so every record is parsed in the SAME stack frame: a
		// million-line document does not grow the rule stack, and each
		// record's parent stays the jsonl node that @push$ appends to.
		"record": {
			Open: []*tabnas.GrammarAltSpec{
				// A separator run that ends the document: nothing more to
				// parse, and nothing to push. Reached when the trailing
				// blank lines were not contiguous — see the next alt.
				{S: "#LN #ZZ", G: "jsonl"},

				// Another separator before any value. A RUN of newline
				// characters lexes as one #LN, but a blank line containing
				// a space does not: "\n \n" lexes as #LN #SP #LN, and #SP
				// is still ignored, so two separators reach the grammar.
				// Skip the extra one and try again, which makes any mix of
				// blank and whitespace-only lines behave the same way.
				{S: "#LN", R: "record", G: "jsonl"},

				// The line itself.
				{P: "val", G: "jsonl"},
			},
			Close: []*tabnas.GrammarAltSpec{
				// A trailing separator at end of input: the file ends with
				// a newline, which is conventional and adds no record.
				{S: "#LN #ZZ", A: "@push$", G: "jsonl,end"},

				// A separator with more to come: iterate to the next record.
				{S: "#LN", R: "record", A: "@push$", G: "jsonl,end"},

				// End of input with no trailing newline.
				{S: "#ZZ", A: "@push$", G: "jsonl,end"},
			},
		},
	}

	// RuleOrder is declared because a Go map has none: without it the
	// engine falls back to sorted rule names, and anything built on
	// RuleNames would report them alphabetically rather than as written.
	return j.Grammar(&tabnas.GrammarSpec{
		V:         2,
		Rule:      rules,
		RuleOrder: []string{"jsonl", "record"},
	})
}

// Jsonl is the standard plugin form. Install it on an engine that ALREADY
// has the strict-JSON grammar:
//
//	j := tabnas.Make()
//	tabnasjson.Json(j, nil)
//	tabnasjsonl.Jsonl(j, nil)
//
// Order matters, and not only by convention: the json plugin sets
// rule.Include "json", which would filter this plugin's alternates
// straight back out if it were applied afterwards. Applying the json
// plugin here when it is absent would silently accept the wrong order, so
// instead the missing-grammar case is reported.
func Jsonl(j *tabnas.Tabnas, _ map[string]any) error {
	if err := checkStrictJSONBase(j); err != nil {
		return err
	}
	j.SetOptions(jsonlOptions())
	return RegisterJsonlGrammar(j)
}

// checkStrictJSONBase reports whether the engine carries a STRICT-JSON value
// grammar to layer on.
//
// Testing for a val rule alone is not enough: every JSON-family grammar
// defines one, so installing over a relaxed grammar would pass such a check
// and then happily accept {a:1} — a document this package's own
// documentation says is invalid. What matters is not which package
// installed the rules but whether a record's CONTENT is standard JSON, so
// the check reads the three lexer options that decide exactly that. A
// relaxed grammar fails at least one of them (jsonic lexes bare text, lexes
// comments, and accepts ' and backtick strings), and the error names the
// ones that are wrong.
func checkStrictJSONBase(j *tabnas.Tabnas) error {
	if !hasRule(j, "val") {
		return fmt.Errorf(
			"tabnasjsonl: the strict-JSON grammar must be installed first — " +
				"call tabnasjson.Json(j, nil) before Jsonl(j, nil), or use Make()")
	}

	opts := j.Options()
	var relaxed []string
	if opts.Text == nil || opts.Text.Lex == nil || *opts.Text.Lex {
		relaxed = append(relaxed, "text.lex")
	}
	if opts.Comment == nil || opts.Comment.Lex == nil || *opts.Comment.Lex {
		relaxed = append(relaxed, "comment.lex")
	}
	if opts.String == nil || opts.String.Chars != `"` {
		relaxed = append(relaxed, "string.chars")
	}

	if 0 < len(relaxed) {
		return fmt.Errorf(
			"tabnasjsonl: the installed value grammar is not strict JSON (%s), "+
				"so records would not be standard JSON. Layer this plugin on "+
				"github.com/tabnas/json/go, not on a relaxed grammar such as jsonic",
			strings.Join(relaxed, ", "))
	}
	return nil
}

// hasRule reports whether the named rule is defined on j.
func hasRule(j *tabnas.Tabnas, name string) bool {
	for _, n := range j.RuleNames() {
		if n == name {
			return true
		}
	}
	return false
}

// Make builds a JSON Lines parser instance: a tabnas engine with the
// strict-JSON grammar and this plugin installed, in that order. Extra
// options are applied after the grammar exists, mirroring the TS make().
func Make(extra ...tabnas.Options) *tabnas.Tabnas {
	j := tabnas.Make()
	if err := tabnasjson.Json(j, nil); err != nil {
		// The JSON grammar spec is fixed and valid, so this only fires on
		// a programmer error in the dependency.
		panic(err)
	}
	if err := Jsonl(j, nil); err != nil {
		// Likewise: the grammar spec above is fixed, and the strict-JSON
		// base was just installed.
		panic(err)
	}
	// Extra options are applied after the grammar exists so that rule
	// include/exclude filters operate on the installed alternates.
	for _, o := range extra {
		j.SetOptions(o)
	}
	return j
}

// defaultParser is a lazily-created instance reused by Parse, so repeated
// calls don't rebuild the engine and grammar each time. Parsing builds a
// fresh context per call and only reads instance state, so the shared
// instance is safe for concurrent use.
var (
	defaultOnce   sync.Once
	defaultParser *tabnas.Tabnas
)

// Parse parses a JSON Lines source string and returns the slice of
// per-line values, or a *tabnas.TabnasError on failure. The error's Row is
// the line of the offending record.
func Parse(src string) (any, error) {
	defaultOnce.Do(func() { defaultParser = Make() })
	return defaultParser.Parse(src)
}
