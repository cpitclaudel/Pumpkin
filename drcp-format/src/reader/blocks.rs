//! Reading croc proofs in the block format (experimental), as text or in the binary form, into a
//! [`BlockProofWriter`].

use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::num::NonZero;

use crate::IntComparison::*;
use crate::writer::blocks::BINARY_MARK;
use crate::writer::blocks::BlockProofWriter;

/// Read a croc proof in the block format, in the binary form when it starts with its mark and as
/// text otherwise, and write it to `sink`, in the binary form if `binary`: each text line as the
/// writer writes a line, and each binary record as the writer writes what it decodes to.
pub fn convert(mut source: impl BufRead, sink: impl Write, binary: bool) -> std::io::Result<()> {
    let mut writer = if binary {
        BlockProofWriter::<_, i64>::new_binary(sink)?
    } else {
        BlockProofWriter::new(sink)
    };
    if !source.fill_buf()?.starts_with(BINARY_MARK) {
        let mut line = String::new();
        while source.read_line(&mut line)? != 0 {
            writer.log_line(line.strip_suffix('\n').unwrap_or(&line))?;
            line.clear();
        }
        return Ok(());
    }
    source.consume(BINARY_MARK.len());
    let mut names = vec![];
    let mut codes = vec![];
    let mut hint = vec![];
    let mut tag = [0];
    while source.read(&mut tag)? != 0 {
        match tag[0] {
            0 => writer.log_line(&read_string(&mut source)?)?,
            1 => names.push(read_string(&mut source)?),
            2 => {
                let reference = reference(read_number(&mut source)?)?;
                let name = names
                    .get(read_number::<usize>(&mut source)?.wrapping_sub(1))
                    .ok_or_else(|| invalid("an undeclared variable name"))?;
                let mut comparator = [0];
                source.read_exact(&mut comparator)?;
                let comparison = *[LessEqual, GreaterEqual, Equal, NotEqual]
                    .get(usize::from(comparator[0]))
                    .ok_or_else(|| invalid("an unknown comparator"))?;
                writer.write_atomic(reference, name, comparison, read_signed(&mut source)?)?;
            }
            3 | 4 => {
                let premise_free = tag[0] == 4;
                let length = if premise_free {
                    1
                } else {
                    read_varint(&mut source)?
                };
                codes.clear();
                for _ in 0..length {
                    codes.push(reference(read_signed(&mut source)?)?);
                }
                let constraint = NonZero::new(read_number(&mut source)?);
                let label =
                    NonZero::new(read_number(&mut source)?).ok_or_else(|| invalid("label 0"))?;
                hint.clear();
                for _ in 0..read_varint(&mut source)? {
                    hint.push(read_number(&mut source)?);
                }
                if premise_free {
                    writer.write_premise_free_inference(codes[0], constraint, label, &hint)?;
                } else {
                    writer.write_inference(None, &codes, constraint, label, &hint)?;
                }
            }
            _ => return Err(invalid("an unknown record tag")),
        }
    }
    Ok(())
}

fn invalid(reason: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, reason)
}

fn reference(value: i64) -> std::io::Result<NonZero<i32>> {
    i32::try_from(value)
        .ok()
        .and_then(NonZero::new)
        .ok_or_else(|| invalid("an atomic constraint reference out of range"))
}

/// Read a number in LEB128.
fn read_varint(source: &mut impl Read) -> std::io::Result<u64> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let mut byte = [0];
        source.read_exact(&mut byte)?;
        value |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(invalid("a number longer than 64 bits"))
}

fn read_number<T: TryFrom<u64>>(source: &mut impl Read) -> std::io::Result<T> {
    T::try_from(read_varint(source)?).map_err(|_| invalid("a number out of range"))
}

/// Read a number zigzag encoded, in LEB128.
fn read_signed(source: &mut impl Read) -> std::io::Result<i64> {
    let value = read_varint(source)?;
    Ok(((value >> 1) as i64) ^ -((value & 1) as i64))
}

fn read_string(source: &mut impl Read) -> std::io::Result<String> {
    let mut bytes = vec![0; read_number(source)?];
    source.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| invalid("a string that is not UTF-8"))
}
