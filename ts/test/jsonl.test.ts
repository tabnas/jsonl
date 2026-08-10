/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

// In-language tests for @tabnas/jsonl.
//
// Parse cases that are expressible as `input -> JSON` live in the shared
// `test/spec/*.tsv` fixtures instead, so they run in BOTH runtimes (see
// ../../test/AGENTS.md). What is left here is what a fixture cannot state:
// the package's API surface, error metadata, the layering contract, and
// scale.

import { describe, test } from 'node:test'
import assert from 'node:assert'

import { Tabnas, TabnasError } from '@tabnas/parser'
import { json } from '@tabnas/json'
import parse, { jsonl, make, registerJsonlGrammar, JsonlError, VERSION } from '../dist/jsonl'

// Objects come back with a null prototype (the engine builds maps that way
// so a `__proto__` key cannot poison the result), which strict deep-equal
// counts as a difference. Round-trip through JSON to compare shape, the same
// normalisation the shared-fixture runner uses.
const plain = (v: any) => JSON.parse(JSON.stringify(v))

describe('api', () => {
  test('parse returns an array of per-line values', () => {
    assert.deepStrictEqual(plain(parse('{"a":1}\n{"b":2}')), [{ a: 1 }, { b: 2 }])
  })

  test('objects are built with a null prototype', () => {
    // Documented behaviour inherited from @tabnas/json, not incidental:
    // a record whose key is `__proto__` stays ordinary data.
    const [record] = parse('{"__proto__":{"polluted":true}}')
    assert.equal(Object.getPrototypeOf(record), null)
    assert.equal(({} as any).polluted, undefined)
  })

  test('parse is also a named export, and the default export', () => {
    const named = require('../dist/jsonl').parse
    assert.equal(typeof named, 'function')
    assert.deepStrictEqual(named('1'), [1])
  })

  test('a single record still yields an array, not a bare value', () => {
    const out = parse('{"a":1}')
    assert.ok(Array.isArray(out), 'expected an array')
    assert.equal(out.length, 1)
  })

  test('repeated parse calls reuse one engine without leaking state', () => {
    const first = parse('{"a":1}\n{"b":2}')
    const second = parse('{"c":3}')
    assert.deepStrictEqual(plain(first), [{ a: 1 }, { b: 2 }])
    assert.deepStrictEqual(plain(second), [{ c: 3 }])
  })

  test('each parse returns a fresh array', () => {
    const a = parse('1')
    const b = parse('1')
    assert.notStrictEqual(a, b, 'results must not be a shared instance')
    a.push('mutated')
    assert.deepStrictEqual(parse('1'), [1])
  })

  test('make() builds an independent instance', () => {
    const one = make()
    const two = make()
    assert.notStrictEqual(one, two)
    assert.deepStrictEqual(one.parse('1\n2'), [1, 2])
  })

  test('VERSION is exported and semver-shaped', () => {
    assert.match(VERSION, /^\d+\.\d+\.\d+/)
  })

  test('JsonlError is the engine error type', () => {
    assert.equal(JsonlError, TabnasError)
  })
})

describe('layering on @tabnas/json', () => {
  test('use(json).use(jsonl) is the supported composition', () => {
    const tn = new Tabnas().use(json).use(jsonl)
    assert.deepStrictEqual(plain(tn.parse('{"a":1}\n{"b":2}')), [{ a: 1 }, { b: 2 }])
  })

  test('installing on a bare engine reports the missing base grammar', () => {
    assert.throws(
      () => new Tabnas().use(jsonl),
      /strict-JSON grammar must be installed first/,
      'expected a named error, not an obscure parse failure later',
    )
  })

  test('the strict-JSON rules are reused, not redefined', () => {
    // `registerJsonlGrammar` adds exactly the two document rules; the five
    // JSON rules must already be there and must survive.
    const tn = new Tabnas().use(json)
    const before = Object.keys(tn.rule() as object).sort()
    assert.deepStrictEqual(before, ['elem', 'list', 'map', 'pair', 'val'])

    tn.options({
      tokenSet: { IGNORE: ['#SP', null, '#CM'] },
      rule: { start: 'jsonl', include: 'json,jsonl' },
    })
    registerJsonlGrammar(tn)

    const after = Object.keys(tn.rule() as object).sort()
    assert.deepStrictEqual(after, [
      'elem', 'jsonl', 'list', 'map', 'pair', 'record', 'val',
    ])
    assert.deepStrictEqual(plain(tn.parse('{"a":1}\n2')), [{ a: 1 }, 2])
  })

  test('a record is parsed by the inherited JSON value grammar', () => {
    // Anything @tabnas/json accepts as a value is accepted as a record.
    const value = '{"s":"x","n":-1.5e2,"t":true,"f":false,"z":null,"l":[1,[2]],"m":{"k":{}}}'
    assert.deepStrictEqual(plain(parse(value)), [JSON.parse(value)])
  })
})

describe('errors', () => {
  test('a bad record reports the line it is on', () => {
    try {
      parse('{"a":1}\n{"b":}\n{"c":3}')
      assert.fail('expected a parse error')
    } catch (err: any) {
      assert.equal(err.lineNumber, 2, 'error should point at the second record')
      assert.ok(err instanceof TabnasError)
      // `code` is carried on the error but not in its public .d.ts.
      assert.equal(typeof (err as any).code, 'string')
    }
  })

  test('line numbers survive many preceding records', () => {
    const src = Array.from({ length: 50 }, (_, i) => `{"i":${i}}`).join('\n')
    try {
      parse(src + '\nnot-json')
      assert.fail('expected a parse error')
    } catch (err: any) {
      assert.equal(err.lineNumber, 51)
    }
  })

  test('a value split across lines fails on the line it breaks', () => {
    try {
      parse('{"a":1}\n{"b":\n2}')
      assert.fail('expected a parse error')
    } catch (err: any) {
      assert.equal(err.lineNumber, 2)
    }
  })

  test('empty source is rejected, as it is by the strict-JSON base', () => {
    // Inherited from @tabnas/json (`lex.empty: false`), matching
    // JSON.parse(''). A document of only blank lines is a different case:
    // it holds zero records and parses to []. Documented in the README.
    assert.throws(() => parse(''), TabnasError)
    assert.deepStrictEqual(parse('\n'), [])
  })
})

describe('scale', () => {
  test('a large document parses without growing the rule stack', () => {
    // `record` iterates by REPLACE, so depth is constant in the record
    // count. A push-based loop would overflow well before this.
    const n = 20000
    const src = Array.from({ length: n }, (_, i) => `{"i":${i}}`).join('\n')
    const out = parse(src)
    assert.equal(out.length, n)
    assert.deepStrictEqual(plain(out[0]), { i: 0 })
    assert.deepStrictEqual(plain(out[n - 1]), { i: n - 1 })
  })

  test('deep nesting inside one record still works', () => {
    const depth = 200
    const src = '['.repeat(depth) + ']'.repeat(depth)
    const out = parse(src)
    assert.equal(out.length, 1)
    let node = out[0]
    let seen = 1
    while (Array.isArray(node) && node.length > 0) {
      node = node[0]
      seen++
    }
    assert.equal(seen, depth)
  })
})
