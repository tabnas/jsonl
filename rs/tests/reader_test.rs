// A parse from a reader (`Tabnas::parse_reader`, admin ADR-30). `make()`
// declares `lex.window`, so a reader is read a window at a time. Every row
// of the shared fixtures parses from a reader, handed over three bytes at a
// time, to the row's expected value, as `parity_test.rs` parses its text,
// and a long file of records parses the same both ways.

mod common;

use std::io::Read;
use std::path::Path;

use tabnas_support::{find_spec_dir, Failure, Runner, Value};

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

#[test]
fn make_reads_a_reader_a_window_at_a_time() {
    assert!(tabnas_jsonl::make().options.lex.window);
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
fn a_long_file_of_records_parses_the_same_from_a_reader() {
    let records: Vec<String> = (0..20_000)
        .map(|i| format!("{{\"id\":{i},\"name\":\"ñ{i}\",\"tags\":[\"a\",\"b\"]}}"))
        .collect();
    let text = records.join("\n") + "\n";
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
}
