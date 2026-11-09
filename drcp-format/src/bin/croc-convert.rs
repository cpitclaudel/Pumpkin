//! `croc-convert <input> <output>`: convert a croc proof in the block format between its text and
//! binary forms; the output is binary when its path ends in `.crocb`.

use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;

use drcp_format::reader::blocks::convert;

fn main() -> std::io::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let [_, input, output] = &args[..] else {
        panic!("usage: croc-convert <input> <output>");
    };
    let mut sink = BufWriter::new(File::create(output)?);
    convert(
        BufReader::new(File::open(input)?),
        &mut sink,
        output.ends_with(".crocb"),
    )?;
    sink.flush()
}
