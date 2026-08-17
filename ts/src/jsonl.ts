/* Copyright (c) 2026 tabnas, MIT License */

/*  jsonl.ts
 *
 *  A JSON Lines (JSONL, also known as NDJSON) grammar plugin for the
 *  `tabnas` parsing engine.
 *
 *  JSON Lines (https://jsonlines.org) is a text format where each line is
 *  one complete, standard-JSON value, and the newline is the record
 *  separator. A document parses to an array of the per-line values.
 *
 *      {"name":"alice","age":30}
 *      {"name":"bob","age":25}
 *
 *      => [{name:'alice',age:30}, {name:'bob',age:25}]
 *
 *  This plugin is a deliberately small demonstration of the engine's
 *  extensible-grammar model: it adds NO lexer matchers and re-uses the
 *  entire strict-JSON rule set (val / map / list / pair / elem) from
 *  `@tabnas/json` untouched. All of JSONL is expressed as
 *
 *    1. one lexer semantic change — the newline token stops being
 *       ignorable and becomes a meaningful token the grammar can match;
 *    2. two new rules — `jsonl` (the document) and `record` (one line).
 *
 *  Point 1 does more work than it appears to. Once `#LN` is no longer in
 *  the IGNORE token set, a newline inside a record is no longer invisible
 *  to the parser, so a value SPLIT ACROSS LINES stops parsing — which is
 *  exactly the JSON Lines requirement that each record occupy one line.
 *  That rule is not written down anywhere below; it falls out of making
 *  the separator significant.
 */

import { Tabnas, TabnasError, type Plugin } from '@tabnas/parser'
import { json } from '@tabnas/json'

// VERSION is this package's version. It MUST equal package.json "version":
// the release orchestrator rewrites both, and test/version.test.ts fails the
// build if they drift. Mirrors `const VERSION` in go/jsonl.go.
export const VERSION = '0.1.4'

// JSONL options, applied over the strict-JSON base.
//
// Everything that makes a record's CONTENT strict JSON — double-quoted
// strings, plain decimal numbers, quoted keys, no comments, no trailing
// commas — is inherited from `@tabnas/json` and deliberately not restated
// here.
const JSONL_OPTIONS = {
  // The one lexer semantic change. The default IGNORE set is
  // ['#SP','#LN','#CM']: the lexer emits those tokens and the parser skips
  // them between meaningful tokens. Dropping '#LN' hands newlines to the
  // grammar, which is what lets `record` use one as a separator.
  //
  // NOTE the shape: the TS engine merges tokenSet INDEX-WISE against the
  // default, so a slot is cleared with an explicit `null` rather than by
  // writing a shorter array. (The Go engine replaces the set outright —
  // see go/jsonl.go, which lists the survivors instead. Same result, and
  // go/doc/concepts.md explains why the two spellings differ.)
  tokenSet: { IGNORE: ['#SP', null, '#CM'] },

  rule: {
    // Parse a whole document, not a single value.
    start: 'jsonl',

    // `@tabnas/json` narrows the active alternates to its own `json` tag;
    // widen that to admit this plugin's alternates too. This is why the
    // json plugin must be installed BEFORE this one — see the `jsonl`
    // plugin function below.
    include: 'json,jsonl',
  },
}

// Install the JSONL document rules on the given engine instance. Exposed
// separately from the options (the same split `@tabnas/json` makes) so a
// plugin layering on JSONL can re-use the rules without re-declaring them.
//
// The value tree is built entirely by the engine's native-value
// `$`-builtins, so this grammar is function-free and serializable:
//
//   @array$ — allocate an empty array into the node (the document).
//   @push$  — append the just-built child value to that array.
export function registerJsonlGrammar(tn: Tabnas): void {
  tn.grammar({
    // The schema version of the native-value builtins this grammar binds
    // to, matching the strict-JSON grammar it layers on.
    v: 2,

    rule: {
      // jsonl: the whole document — a possibly-empty sequence of records.
      // It allocates the result array and hands off to `record`, which
      // iterates. The engine's line matcher scans a RUN of line characters
      // into a single `#LN` token and treats CRLF as one newline, so
      // contiguous blank lines cost nothing here; blank lines that contain
      // whitespace are handled by `record` (see its open alternates).
      jsonl: {
        open: [
          // A document of only blank lines (or, with `lex.empty`, none at
          // all) is a document of zero records.
          { s: '#ZZ', a: '@array$', g: 'jsonl' },
          { s: '#LN #ZZ', a: '@array$', g: 'jsonl' },

          // Leading blank line(s), then the first record.
          { s: '#LN', p: 'record', a: '@array$', g: 'jsonl' },

          // The ordinary case: the first record starts immediately.
          { p: 'record', a: '@array$', g: 'jsonl' },
        ],
        close: [
          // `record` consumes through end-of-input, so there is nothing
          // left to match here.
          { g: 'jsonl' },
        ],
      },

      // record: exactly one line. Pushing `val` re-uses the whole
      // strict-JSON value grammar, so a record may be any JSON value —
      // object, array, string, number, true/false/null — as the JSON Lines
      // format allows.
      //
      // The close alternates iterate with `r` (replace) rather than `p`
      // (push), so every record is parsed in the SAME stack frame: a
      // million-line document does not grow the rule stack, and each
      // record's parent stays the `jsonl` node that @push$ appends to.
      record: {
        open: [
          // A separator run that ends the document: nothing more to
          // parse, and nothing to push. Reached when the trailing blank
          // lines were not contiguous — see the next alternate.
          { s: '#LN #ZZ', g: 'jsonl' },

          // Another separator before any value. A RUN of newline
          // characters lexes as one `#LN`, but a blank line containing a
          // space does not: `"\n \n"` lexes as `#LN #SP #LN`, and `#SP`
          // is still ignored, so two separators reach the grammar. Skip
          // the extra one and try again, which makes any mix of blank and
          // whitespace-only lines behave the same way.
          { s: '#LN', r: 'record', g: 'jsonl' },

          // The line itself.
          { p: 'val', g: 'jsonl' },
        ],
        close: [
          // A trailing separator at end of input: the file ends with a
          // newline, which is conventional and adds no record.
          { s: '#LN #ZZ', a: '@push$', g: 'jsonl,end' },

          // A separator with more to come: iterate to the next record.
          { s: '#LN', r: 'record', a: '@push$', g: 'jsonl,end' },

          // End of input with no trailing newline.
          { s: '#ZZ', a: '@push$', g: 'jsonl,end' },
        ],
      },
    },
  })
}

// The standard plugin form. Install it on an engine that ALREADY has the
// strict-JSON grammar, mirroring how `@tabnas/zon` layers on
// `@tabnas/jsonic`:
//
//     new Tabnas().use(json).use(jsonl)
//
// Order matters, and not only by convention: `@tabnas/json` sets
// `rule.include: 'json'`, which would filter this plugin's alternates
// straight back out if it were applied afterwards. Applying `json` here
// when it is absent would silently accept the wrong order, so instead the
// missing-grammar case is reported.
export const jsonl: Plugin = function jsonl(tn: Tabnas, _options?: any) {
  assertStrictJsonBase(tn)
  tn.options(JSONL_OPTIONS)
  registerJsonlGrammar(tn)
}

// Check that the engine carries a STRICT-JSON value grammar before layering
// on it.
//
// Testing for a `val` rule alone is not enough: every JSON-family grammar
// defines one, so `use(jsonic).use(jsonl)` would pass such a check and then
// happily accept `{a:1}` — a document this package's own documentation says
// is invalid. What matters is not which package installed the rules but
// whether a record's CONTENT is standard JSON, so the check reads the three
// lexer options that decide exactly that. A relaxed grammar fails at least
// one of them (jsonic lexes bare text, lexes comments, and accepts `'` and
// backtick strings), and the error names the ones that are wrong.
function assertStrictJsonBase(tn: Tabnas): void {
  // `tn.rule(name)` returns the instance itself when the rule is absent (it
  // chains), so ask for the whole rule map and look in that instead.
  const rules = tn.rule() as Record<string, unknown>
  if (!rules || !rules.val) {
    throw new Error(
      '@tabnas/jsonl: the strict-JSON grammar must be installed first — ' +
      "use `new Tabnas().use(json).use(jsonl)`, or call this package's " +
      '`make()`.',
    )
  }

  const opts = tn.options() as any
  const relaxed: string[] = []
  if (false !== opts?.text?.lex) relaxed.push('text.lex')
  if (false !== opts?.comment?.lex) relaxed.push('comment.lex')
  if ('"' !== opts?.string?.chars) relaxed.push('string.chars')

  if (0 < relaxed.length) {
    throw new Error(
      '@tabnas/jsonl: the installed value grammar is not strict JSON (' +
      relaxed.join(', ') +
      '), so records would not be standard JSON. Layer this plugin on ' +
      '`@tabnas/json`, not on a relaxed grammar such as `@tabnas/jsonic`.',
    )
  }
}

// Create a JSON Lines parser instance: a tabnas engine with the strict-JSON
// grammar and this plugin installed, in that order. Extra options are
// applied after the grammar exists, mirroring the Go `Make`.
export function make(opts?: Record<string, any>): Tabnas {
  const tn = new Tabnas({ plugins: [json, jsonl] })
  if (opts) {
    tn.options(opts)
  }
  return tn
}

// A lazily-created default instance reused by `parse`, so repeated calls
// don't rebuild the engine and grammar each time. Parsing creates a fresh
// context per call, so reuse is safe.
let defaultParser: Tabnas | undefined

// Parse a JSON Lines source string and return the array of per-line values.
// Throws a TabnasError on invalid input; the error's `lineNumber` is the
// line of the offending record.
export function parse(src: string): any[] {
  return (defaultParser ??= make()).parse(src)
}

export { Tabnas, TabnasError }
export { TabnasError as JsonlError }
export default parse
