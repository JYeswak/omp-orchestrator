use super::{hash_sha256_reader, SHA256_BUFFER_SIZE};
use std::io::{self, Read};
use std::path::Path;

struct ShortReader {
    data: Vec<u8>,
    offset: usize,
    max_chunk: usize,
    requested_sizes: Vec<usize>,
}

impl Read for ShortReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.requested_sizes.push(buffer.len());
        if self.offset == self.data.len() {
            return Ok(0);
        }
        let remaining = self.data.len() - self.offset;
        let bytes = remaining.min(self.max_chunk).min(buffer.len());
        buffer[..bytes].copy_from_slice(&self.data[self.offset..self.offset + bytes]);
        self.offset += bytes;
        Ok(bytes)
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "synthetic read failure"))
    }
}

#[test]
fn sha256_reader_seam_proves_fixed_buffer_short_reads() {
    let mut reader = ShortReader {
        data: b"B03 fixed buffer SHA-256 payload\n".to_vec(),
        offset: 0,
        max_chunk: 3,
        requested_sizes: Vec::new(),
    };
    let actual = hash_sha256_reader(Path::new("short-reader"), &mut reader)
        .expect("short reader digest");
    assert_eq!(
        actual,
        "bdd2a7291457c6a5e371324772061f751ba7774d8d7057895f5d5ea8daa773f1"
    );
    assert!(reader.requested_sizes.len() > 1, "reader must provide short chunks");
    assert!(
        reader
            .requested_sizes
            .iter()
            .all(|size| *size == SHA256_BUFFER_SIZE),
        "all reads must use the fixed buffer: {:?}",
        reader.requested_sizes
    );
    let error = hash_sha256_reader(Path::new("reader-error"), FailingReader)
        .expect_err("reader error must refuse");
    assert_eq!(
        error.to_string(),
        "L0_SHA256_REFUSED: read failure path=reader-error: synthetic read failure"
    );
}
