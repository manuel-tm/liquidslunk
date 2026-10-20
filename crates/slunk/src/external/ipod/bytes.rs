//! Little-endian chunks, as the iPod's databases are made of: a four-letter
//! tag, its header's length, and usually its whole length, written first as
//! a placeholder and filled in once what it holds is written.

use crate::{Error, Result};

/// Bytes being written.
#[derive(Debug, Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer::default()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn tag(&mut self, tag: &[u8; 4]) {
        self.bytes.extend_from_slice(tag);
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn zeros(&mut self, count: usize) {
        self.bytes.resize(self.bytes.len() + count, 0);
    }

    pub fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Pads what's written since `start` with zeros to `length` bytes, for a
    /// header written shorter than the length it declares.
    pub fn pad_to(&mut self, start: usize, length: usize) {
        let written = self.bytes.len() - start;
        debug_assert!(written <= length, "a header longer than declared");
        self.zeros(length.saturating_sub(written));
    }

    /// Writes a u32 at a position already written: a length, or a count,
    /// once it's known.
    pub fn set_u32(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// Fills in a chunk's whole length, at its usual place, 8 bytes in,
    /// from where it starts to what's written now.
    pub fn close(&mut self, start: usize) {
        let length = (self.bytes.len() - start) as u32;
        self.set_u32(start + 8, length);
    }
}

/// Bytes being read. Every read is checked against their end, and fails as
/// a damaged file rather than panicking.
#[derive(Debug, Clone, Copy)]
pub struct Reader<'a> {
    pub bytes: &'a [u8],
    /// What a failure names: "iTunesDB"
    pub file: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8], file: &'static str) -> Reader<'a> {
        Reader { bytes, file }
    }

    pub fn damaged(&self, what: impl std::fmt::Display) -> Error {
        Error::Invalid(format!("The iPod's {} can't be read: {what}", self.file))
    }

    fn slice(&self, at: usize, length: usize) -> Result<&'a [u8]> {
        at.checked_add(length)
            .and_then(|end| self.bytes.get(at..end))
            .ok_or_else(|| self.damaged(format!("it ends before byte {}", at + length)))
    }

    pub fn tag(&self, at: usize) -> Result<[u8; 4]> {
        Ok(self.slice(at, 4)?.try_into().expect("four bytes"))
    }

    pub fn is(&self, at: usize, tag: &[u8; 4]) -> bool {
        self.tag(at).is_ok_and(|found| &found == tag)
    }

    /// Fails unless the chunk at `at` has this tag.
    pub fn expect(&self, at: usize, tag: &[u8; 4]) -> Result<()> {
        if self.is(at, tag) {
            Ok(())
        } else {
            Err(self.damaged(format!(
                "byte {at} isn't an {} chunk",
                String::from_utf8_lossy(tag)
            )))
        }
    }

    pub fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.slice(at, 1)?[0])
    }

    pub fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.slice(at, 2)?.try_into().expect("two bytes"),
        ))
    }

    pub fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.slice(at, 4)?.try_into().expect("four bytes"),
        ))
    }

    pub fn i32(&self, at: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(
            self.slice(at, 4)?.try_into().expect("four bytes"),
        ))
    }

    pub fn u64(&self, at: usize) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.slice(at, 8)?.try_into().expect("eight bytes"),
        ))
    }

    pub fn bytes(&self, at: usize, length: usize) -> Result<&'a [u8]> {
        self.slice(at, length)
    }

    /// A chunk's header length, 4 bytes in, which is at least its tag and
    /// itself.
    pub fn header_length(&self, at: usize) -> Result<usize> {
        let length = self.u32(at + 4)? as usize;
        if length < 8 {
            return Err(self.damaged(format!("the chunk at byte {at} has no length")));
        }
        Ok(length)
    }

    /// A chunk's whole length, 8 bytes in, which holds at least its header,
    /// and ends within the bytes.
    pub fn total_length(&self, at: usize) -> Result<usize> {
        let total = self.u32(at + 8)? as usize;
        if total < self.header_length(at)? {
            return Err(self.damaged(format!("the chunk at byte {at} is shorter than its header")));
        }
        if at + total > self.bytes.len() {
            return Err(self.damaged(format!("the chunk at byte {at} goes past its end")));
        }
        Ok(total)
    }
}

/// Text as UTF-16LE, as the iTunesDB keeps it.
pub fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// UTF-16LE bytes as text, with anything that isn't valid replaced.
pub fn from_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_are_read_back_and_checked() {
        let mut writer = Writer::new();
        writer.tag(b"mhxx");
        writer.u32(12);
        writer.u32(0);
        writer.u16(0xbeef);
        writer.close(0);
        let reader = Reader::new(&writer.bytes, "test");
        assert!(reader.is(0, b"mhxx"));
        assert_eq!(reader.total_length(0).unwrap(), 14);
        assert_eq!(reader.u16(12).unwrap(), 0xbeef);
        assert!(reader.u32(12).is_err());
        assert!(reader.expect(0, b"mhyy").is_err());
        assert_eq!(from_utf16(&utf16("初夏 Shoka")), "初夏 Shoka");
    }
}
