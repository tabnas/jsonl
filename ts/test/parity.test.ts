/* Copyright (c) 2025 Richard Rodger and other contributors, MIT License */

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the escape codec, the `ERROR:<code>` contract and the
// row loop all come from @tabnas/support, whose Go and Rust halves
// `go/parity_test.go` and `rs/tests/parity_test.rs` use to run the SAME
// files — so the implementations cannot drift without one of them going
// red, and neither can the loaders.
//
// What is left here is only what is specific to jsonl.

import { findSpecDir, makeRunner } from '@tabnas/support'

import { make } from '../dist/jsonl'

makeRunner({
  parse: (input, row) => {
    // The `opts` column is part of the shared fixture format, but this
    // plugin takes no options. Rather than silently ignore a value, say
    // so — a fixture author who sets one deserves to be told it would
    // have had no effect.
    const opts = row.named('opts')
    if ('' !== opts.trim()) {
      throw new Error(
        `${row.where()}: opts ${JSON.stringify(opts)} given, but ` +
        '@tabnas/jsonl has no options')
    }

    return make().parse(input)
  },
})
  // `findSpecDir` walks up from this file — `dist-test/` at runtime — to the
  // repo root's `test/spec`, so moving the suite does not mean recounting
  // `..` hops. `dir` then auto-discovers every fixture in it, so adding a
  // .tsv runs it in every runtime without touching any runner.
  .dir(findSpecDir(__dirname))
