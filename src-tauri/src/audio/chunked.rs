use std::io::{Result, Write};

/// Wraps a writer to apply HTTP chunked transfer encoding.
pub(crate) struct ChunkedWriter<W: Write> {
    writer: W,
}

impl<W: Write> ChunkedWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Write for ChunkedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        write!(self.writer, "{:X}\r\n", buf.len())?;
        self.writer.write_all(buf)?;
        write!(self.writer, "\r\n")?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunked_empty_write() {
        let mut buf = Vec::new();
        let mut writer = ChunkedWriter::new(&mut buf);
        let written = writer.write(&[]).unwrap();
        assert_eq!(written, 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_chunked_small_payload() {
        let mut buf = Vec::new();
        let mut writer = ChunkedWriter::new(&mut buf);
        let payload = b"Hello";
        writer.write_all(payload).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("5\r\n"), "Expected chunk size header");
        assert!(output.contains("Hello"));
        assert!(output.ends_with("\r\n"), "Expected chunk terminator");
    }
}
