// Cross-runtime conformance, driven by the shared `test/spec/*.tsv`
// fixtures at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the escape codec, the `ERROR:<code>` contract and
// the row loop all come from tabnas-support, whose TypeScript and Go
// halves `ts/test/parity.test.ts` and `go/parity_test.go` use to run the
// SAME files, so the three implementations cannot drift without one of
// them going red, and neither can the three loaders.
//
// What is left here is only what is specific to jsonl.

mod common;

use std::path::Path;

use tabnas_support::{find_spec_dir, parse_spec, Failure, Runner, SpecOptions, Value};

/// The runner every fixture goes through. Built once per test rather than
/// once per row: the parser is `Send + Sync` and keeps no state between
/// parses, which the in-language suite pins separately.
fn runner() -> Runner {
    let parser = tabnas_jsonl::make();
    Runner::new_with_row(move |input, row| {
        // The `opts` column is part of the shared fixture format, but this
        // plugin takes no options. Rather than silently ignore a value,
        // say so: a fixture author who sets one deserves to be told it
        // would have had no effect. A panic rather than a `Failure`,
        // because a `Failure` on an `ERROR` row would count as a pass.
        let opts = row.named("opts");
        if !opts.trim().is_empty() {
            panic!(
                "{}: opts {opts:?} given, but tabnas-jsonl has no options",
                row.location()
            );
        }

        parser
            .parse(input)
            .map(|value| Value::from(common::plain(&value)))
            .map_err(|error| Failure::new(error.code.clone()).with_message(error.to_string()))
    })
}

/// Every fixture in the spec directory. `find_spec_dir` walks up from the
/// crate directory, and `dir` discovers the files by listing, so adding a
/// .tsv runs it in every runtime without touching any runner. An empty
/// directory, and an empty fixture, both fail inside the runner.
#[test]
fn spec() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    runner().dir(&dir);
}

/// The census this suite is expected to cover. A fixture renamed or
/// removed would otherwise be a silent loss of coverage: `dir` runs what
/// it finds, and finding less is not a failure to it.
#[test]
fn every_fixture_is_present() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    for name in [
        "oneline.tsv",
        "records.tsv",
        "separators.tsv",
        "strict.tsv",
        "values.tsv",
    ] {
        assert!(dir.join(name).is_file(), "missing fixture {name}");
    }
}

/// The guard above has to be seen to fire. A fixture row that sets `opts`
/// fails the run, whatever its expected column says.
#[test]
#[should_panic(expected = "has no options")]
fn an_opts_column_fails_loudly() {
    let spec = parse_spec(
        "with-opts.tsv",
        "input\texpected\topts\n1\t[1]\t{\"x\":true}\n",
        &SpecOptions::default(),
    )
    .expect("the probe fixture loads");
    runner().spec(&spec);
}
