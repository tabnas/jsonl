/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

// Composition test: the JSONL grammar plugin layered with the official
// @tabnas/debug plugin. @tabnas/debug is a devDependency, but this still
// resolves it dynamically and SKIPS when it is absent so the suite stays
// runnable outside the package; TABNAS_DEBUG_PATH can point at a sibling
// checkout's built plugin.
//
// Beyond "does it compose", this test is the structural proof of what the
// plugin actually does: the introspected grammar shows the five strict-JSON
// rules arriving UNCHANGED from @tabnas/json, with exactly two rules added
// on top.

import { describe, test } from 'node:test'
import assert from 'node:assert'

import { Tabnas } from '@tabnas/parser'
import { json } from '@tabnas/json'
import { jsonl } from '../dist/jsonl'

function loadDebug(): any {
  const candidates = [process.env.TABNAS_DEBUG_PATH, '@tabnas/debug'].filter(
    Boolean,
  ) as string[]
  for (const c of candidates) {
    try {
      return require(c).Debug
    } catch {
      /* try next */
    }
  }
  return null
}

const Debug = loadDebug()
const skip = Debug
  ? false
  : '@tabnas/debug not available (set TABNAS_DEBUG_PATH)'

function build(): any {
  const tn = new Tabnas().use(json).use(jsonl, {})
  tn.use(Debug, { print: false, trace: false })
  return tn
}

describe('compose: jsonl + @tabnas/debug', () => {
  test('parses normally with the debug plugin installed', { skip }, () => {
    const tn = build()
    assert.deepStrictEqual(
      JSON.parse(JSON.stringify(tn.parse('{"a":1}\n{"b":[2,3]}'))),
      [{ a: 1 }, { b: [2, 3] }],
    )
  })

  test('debug.model() shows JSON + two added rules', { skip }, () => {
    const tn = build()
    const m = tn.debug.model()

    // The five strict-JSON rules, plus this plugin's two. That the JSON
    // five are present and unrenamed IS the layering claim.
    assert.deepStrictEqual(
      m.rules.map((r: any) => r.name).sort(),
      ['elem', 'jsonl', 'list', 'map', 'pair', 'record', 'val'],
    )

    // The document rule is the entry point, not `val`: this is what makes
    // the parser read a whole file rather than a single value.
    assert.equal(m.config.start, 'jsonl')

    assert.ok(
      m.plugins.some((p: any) => p.name === 'jsonl'),
      'plugins should list jsonl',
    )

    // The rule-reference graph captures the document structure:
    // jsonl -> record -> val, and `record` close-REPLACES itself to iterate.
    // Replace (not push) is why a million-line document parses in a single
    // stack frame.
    const edge = (name: string) => m.graph.find((e: any) => e.name === name)
    assert.deepStrictEqual(edge('jsonl').openPush, ['record'])
    assert.deepStrictEqual(edge('record').openPush, ['val'])
    assert.deepStrictEqual(edge('record').closeReplace, ['record'])

    // The inherited JSON structure is untouched underneath.
    assert.deepStrictEqual(edge('val').openPush.slice().sort(), ['list', 'map'])
    assert.deepStrictEqual(edge('map').openPush, ['pair'])
    assert.deepStrictEqual(edge('list').openPush, ['elem'])
    assert.deepStrictEqual(edge('pair').closeReplace, ['pair'])
    assert.deepStrictEqual(edge('elem').closeReplace, ['elem'])

    // The grammar portion is JSON-serialisable and round-trips.
    const grammar = {
      tokens: m.tokens,
      rules: m.rules,
      graph: m.graph,
      config: m.config,
      abnf: m.abnf,
    }
    assert.deepStrictEqual(JSON.parse(JSON.stringify(grammar)).rules, m.rules)
  })
})
