// A parse from a reader (`Tabnas::parse_reader`, admin ADR-30). `make()`
// declares `lex.window`, so a reader is read a window at a time; the one
// lexer override, `#LN` dropped from IGNORE, changes which tokens the parser
// skips, not how the text is read. Every row of the shared fixtures parses
// from a reader, handed over three bytes at a time, to the row's expected
// value, as `parity_test.rs` parses its text. Every row also parses the
// same from a reader as from its text, handed over in pieces of 1, 3 and 64
// bytes, under the window's own sizes and under windows whose margin and
// block are a few characters (`parse_reader_with_sizes`, which makes every
// matcher meet the window's edge): the same value, or an error with the
// same code, position, row and column. So do a long file of records and a
// file of tokens longer than the window. `ts/test/reader.test.ts` and
// `go/reader_test.go` hold the other runtimes to the same.

mod common;

use std::io::Read;
use std::path::Path;

use tabnas::{Tabnas, WindowSizes};
use tabnas_support::{find_spec_dir, load_spec_dir, Failure, Runner, SpecOptions, Value};

/// The pieces a reader hands its bytes over in.
const PIECES: [usize; 3] = [1, 3, 64];

/// Windows whose margin and block are a few characters.
const WINDOWS: [(usize, usize); 3] = [(1, 1), (3, 2), (64, 64)];

/// A reader that hands its bytes over at most `size` at a time.
struct Pieces<'a> {
    bytes: &'a [u8],
    at: usize,
    size: usize,
}

impl Read for Pieces<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.size.min(buf.len()).min(self.bytes.len() - self.at);
        buf[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

fn pieces(bytes: &[u8], size: usize) -> Pieces<'_> {
    Pieces { bytes, at: 0, size }
}

/// What a parse gave: its value, or its error's code, position, row and
/// column. An error's excerpt is left out: a long line's may be cut where
/// the window begins, the one thing a parse from a reader shows differently.
fn outcome(parsed: Result<tabnas::Value, tabnas::TabnasError>) -> String {
    match parsed {
        Ok(value) => format!("ok {value}"),
        Err(e) => format!("err {} at {} {}:{}", e.code, e.pos, e.row, e.col),
    }
}

/// Every way this test reads `bytes` from a reader, each with its outcome:
/// in every size of pieces under the window's own sizes, and under every
/// small window.
fn readings(parser: &Tabnas, bytes: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for size in PIECES {
        out.push((
            format!("pieces of {size}"),
            outcome(parser.parse_reader(pieces(bytes, size))),
        ));
        for (margin, block) in WINDOWS {
            let sizes = WindowSizes {
                margin,
                block,
                chunk: size,
            };
            out.push((
                format!("pieces of {size}, margin {margin}, block {block}"),
                outcome(parser.parse_reader_with_sizes(pieces(bytes, size), sizes)),
            ));
        }
    }
    out
}

/// A long file of `count` records, one a line.
fn records(count: usize) -> String {
    let records: Vec<String> = (0..count)
        .map(|i| format!("{{\"id\":{i},\"name\":\"ñ{i}\",\"tags\":[\"a\",\"b\"]}}"))
        .collect();
    records.join("\n") + "\n"
}

/// A file of tokens each longer than a window's margin of 64 KiB, so that
/// a window grows to hold one: a record that is a string of escapes and
/// characters past ASCII, a record that is a number, a run of spaces and
/// tabs before a record, and a run of blank lines. `go/reader_test.go`,
/// which cannot make a window small, places a token of every kind across
/// the window's boundaries instead.
fn long_tokens() -> String {
    format!(
        "\"{}\"\n1.{}\n{}true{}null\n",
        "ab\\u00e9ñ€𝄞\\n".repeat(5000),
        "1234567890".repeat(7000),
        " \t".repeat(40000),
        "\n\r\n".repeat(30000),
    )
}

#[test]
fn make_reads_a_reader_a_window_at_a_time() {
    assert!(tabnas_jsonl::make().options.lex.window);
    // The plugin leaves the window to the parser that installs it.
    let mut parser = Tabnas::new();
    tabnas_json::json(&mut parser).expect("the JSON grammar installs");
    tabnas_jsonl::jsonl(&mut parser).expect("the JSONL grammar installs");
    assert!(!parser.options.lex.window);
}

/// The shared fixtures, each row's input from a reader.
#[test]
fn spec_from_a_reader() {
    let parser = tabnas_jsonl::make();
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    Runner::new_with_row(move |input, _row| {
        let reader = Pieces {
            bytes: input.as_bytes(),
            at: 0,
            size: 3,
        };
        parser
            .parse_reader(reader)
            .map(|value| Value::from(common::plain(&value)))
            .map_err(|error| Failure::new(error.code.clone()).with_message(error.to_string()))
    })
    .dir(&dir);
}

#[test]
fn every_row_parses_the_same_from_a_reader() {
    let parser = tabnas_jsonl::make();
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    let mut compared = 0;
    for file in load_spec_dir(&dir, &SpecOptions::default()).expect("the fixtures load") {
        for row in &file.rows {
            let input = row.unesc_named("input");
            let whole = outcome(parser.parse(&input));
            for (how, read) in readings(&parser, input.as_bytes()) {
                assert_eq!(read, whole, "{} in {how}", row.location());
                compared += 1;
            }
        }
    }
    assert!(compared > 1000, "{compared}");
}

#[test]
fn a_long_file_of_records_parses_the_same_from_a_reader() {
    let text = records(20_000);
    let parser = tabnas_jsonl::make();
    let whole = parser.parse(&text).expect("the text parses").to_string();
    let reader = Pieces {
        bytes: text.as_bytes(),
        at: 0,
        size: 4096,
    };
    let read = parser
        .parse_reader(reader)
        .expect("the reader parses")
        .to_string();
    assert_eq!(read, whole);
    // Under a small window, 2,000 records, some 85 KB, since a debug build
    // takes seconds over each parse of the 20,000.
    let text = records(2_000);
    let whole = outcome(parser.parse(&text));
    let small = WindowSizes {
        margin: 16,
        block: 16,
        chunk: 7,
    };
    let cut = outcome(parser.parse_reader_with_sizes(pieces(text.as_bytes(), 5), small));
    assert_eq!(cut, whole);
}

#[test]
fn tokens_longer_than_the_window_parse_the_same_from_a_reader() {
    // As written, with an error after them, and cut inside the string.
    let parser = tabnas_jsonl::make();
    let text = long_tokens();
    let cut: String = text.chars().take(50_000).collect();
    let docs = [
        ("as written", text.clone(), "ok "),
        ("with an error after them", format!("{text}x\n"), "err "),
        ("cut inside the string", cut, "err "),
    ];
    for (what, doc, kind) in docs {
        let whole = outcome(parser.parse(&doc));
        let head: String = whole.chars().take(80).collect();
        assert!(whole.starts_with(kind), "{what}: {head}");
        for size in [4096, 64 * 1024 + 7] {
            let read = outcome(parser.parse_reader(pieces(doc.as_bytes(), size)));
            assert_eq!(read, whole, "{what}, in pieces of {size}");
        }
        let small = WindowSizes {
            margin: 16,
            block: 16,
            chunk: 7,
        };
        let read = outcome(parser.parse_reader_with_sizes(pieces(doc.as_bytes(), 5), small));
        assert_eq!(read, whole, "{what}, in a small window");
    }
}
