/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

// reader.test.ts — a parse from a reader (`tn.parseReader`, admin ADR-30).
//
// `make()` declares `lex.window`, since every token of this grammar, JSON's
// base included, is the engine's own matchers', and JSON's one check reads
// no further than a number's text, so a reader is read a window at a time.
// The one lexer override, `#LN` dropped from IGNORE, changes which tokens
// the parser skips, not how the text is read. Every row of the shared
// fixtures parses the same from a reader, handed over a few bytes at a time
// and under windows whose margin and block are a few units
// (`parseReaderWithSizes`, which makes every matcher meet the window's
// edge), as from its text: the same value, or an error with the same code,
// position, row and column. So do two long documents.
// rs/tests/reader_test.rs and go/reader_test.go hold the other runtimes to
// the same.

import { describe, it } from 'node:test'
import assert from 'node:assert'

import { findSpecDir, loadSpecDir } from '@tabnas/support'
import { json } from '@tabnas/json'
import type { Reader } from '@tabnas/parser'

import { Tabnas, jsonl, make } from '../dist/jsonl'

// The pieces a reader hands its bytes over in.
const PIECES = [1, 3, 64]

// Windows whose margin and block are a few units.
const WINDOWS: [number, number][] = [[1, 1], [3, 2], [64, 64]]

// A reader that hands its bytes over at most `size` at a time.
function pieces(bytes: Uint8Array, size: number): Reader {
  let at = 0
  return {
    read(buf: Uint8Array): number {
      const n = Math.min(size, buf.length, bytes.length - at)
      buf.set(bytes.subarray(at, at + n))
      at += n
      return n
    },
  }
}

// A value as text, with the sign of a negative zero kept and the keys in
// their order, so that two values compare as text.
function render(value: any): string {
  if ('number' === typeof value && Object.is(value, -0)) return '-0'
  if (Array.isArray(value)) return '[' + value.map(render).join(',') + ']'
  if (null !== value && 'object' === typeof value) {
    return '{' + Object.keys(value)
      .map((k) => JSON.stringify(k) + ':' + render(value[k])).join(',') + '}'
  }
  return JSON.stringify(value)
}

// What a parse gave: its value, or its error's code, position, row and
// column. An error's excerpt is left out: a long line's may be cut where
// the window begins, the one thing a parse from a reader shows differently.
function outcome(parse: () => any): string {
  try {
    return 'ok ' + render(parse())
  } catch (e: any) {
    if (null == e || 'function' !== typeof e.toJSON) throw e
    const d = e.toJSON()
    return `err ${d.code} at ${d.pos} ${d.row}:${d.col}`
  }
}

// Every way this test reads `bytes` from a reader, each with its outcome:
// in every size of pieces under the window's own sizes, and under every
// small window.
function readings(tn: Tabnas, bytes: Uint8Array): [string, string][] {
  const out: [string, string][] = []
  for (const size of PIECES) {
    out.push([`pieces of ${size}`,
      outcome(() => tn.parseReader(pieces(bytes, size)))])
    for (const [margin, block] of WINDOWS) {
      out.push([`pieces of ${size}, margin ${margin}, block ${block}`,
        outcome(() => tn.parseReaderWithSizes(
          pieces(bytes, size), { margin, block, chunk: size }))])
    }
  }
  return out
}

// A long file of `count` records, one a line, as rs/tests/reader_test.rs
// writes it.
function records(count: number): string {
  const out: string[] = []
  for (let i = 0; i < count; i++) {
    out.push(`{"id":${i},"name":"ñ${i}","tags":["a","b"]}`)
  }
  return out.join('\n') + '\n'
}

// A file of tokens each longer than a window's margin of 64 KiB, so that a
// window grows to hold one: a record that is a string of escapes and
// characters past ASCII, a record that is a number, a run of spaces and
// tabs before a record, and a run of blank lines. go/reader_test.go, which
// cannot make a window small, places a token of every kind across the
// window's boundaries instead.
function longTokens(): string {
  return '"' + 'ab\\u00e9ñ€𝄞\\n'.repeat(5000) + '"\n' +
    '1.' + '1234567890'.repeat(7000) + '\n' + ' \t'.repeat(40000) +
    'true' + '\n\r\n'.repeat(30000) + 'null\n'
}

describe('reader', () => {
  it('make reads a reader a window at a time', () => {
    assert.equal(make().config().lex.window, true)
    // The plugin leaves the window to the parser that installs it.
    assert.equal(
      new Tabnas().use(json).use(jsonl).config().lex.window, false)
  })

  it('every row parses the same from a reader', () => {
    const tn = make()
    let compared = 0
    for (const file of loadSpecDir(findSpecDir(__dirname))) {
      for (const row of file.rows) {
        const input = row.unescNamed('input')
        const whole = outcome(() => tn.parse(input))
        for (const [how, read] of readings(tn, Buffer.from(input, 'utf8'))) {
          assert.equal(read, whole, `${row.where()} in ${how}`)
          compared++
        }
      }
    }
    assert.ok(1000 < compared, '' + compared)
  })

  it('a long file parses the same from a reader', () => {
    // 5,000 records, some 210 KB, several windows of 64 KiB.
    const tn = make()
    const text = records(5000)
    const bytes = Buffer.from(text, 'utf8')
    const whole = outcome(() => tn.parse(text))
    assert.ok(whole.startsWith('ok '), whole.slice(0, 80))
    assert.equal(outcome(() => tn.parseReader(pieces(bytes, 4096))), whole)
    assert.equal(outcome(() => tn.parseReaderWithSizes(
      pieces(bytes, 5), { margin: 16, block: 16, chunk: 7 })), whole)
  })

  it('tokens longer than the window parse the same from a reader', () => {
    // As written, with an error after them, and cut inside the string.
    const tn = make()
    const text = longTokens()
    const docs: [string, string, string][] = [
      ['as written', text, 'ok '],
      ['with an error after them', text + 'x\n', 'err '],
      ['cut inside the string', text.slice(0, 50000), 'err '],
    ]
    for (const [what, doc, kind] of docs) {
      const bytes = Buffer.from(doc, 'utf8')
      const whole = outcome(() => tn.parse(doc))
      assert.ok(whole.startsWith(kind), `${what}: ${whole.slice(0, 80)}`)
      for (const size of [4096, 64 * 1024 + 7]) {
        assert.equal(outcome(() => tn.parseReader(pieces(bytes, size))),
          whole, `${what}, in pieces of ${size}`)
      }
      assert.equal(outcome(() => tn.parseReaderWithSizes(
        pieces(bytes, 5), { margin: 16, block: 16, chunk: 7 })),
      whole, `${what}, in a small window`)
    }
  })
})
