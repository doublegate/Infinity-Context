//! Versioned, bounds-checked binary snapshot codec for the HDC index (ICX-AUD-001).
//!
//! Persisting the index lets a workspace load in O(read) instead of re-encoding every stored
//! event. The format is little-endian and length-prefixed; every count is validated against a
//! hard cap before allocation, so a corrupted or hostile snapshot fails with an error instead of
//! exhausting memory.

use thiserror::Error;

/// Snapshot file magic (`ICXHDC` + format version).
pub(crate) const SNAPSHOT_MAGIC: &[u8; 8] = b"ICXHDC02";
/// Upper bound on any length-prefixed collection inside a snapshot.
pub(crate) const MAX_SNAPSHOT_ITEMS: u64 = 16 * 1024 * 1024;
/// Upper bound on any length-prefixed string inside a snapshot.
pub(crate) const MAX_SNAPSHOT_STRING: u64 = 64 * 1024;

/// Error decoding an HDC index snapshot.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotError {
    #[error("snapshot magic/version mismatch")]
    BadMagic,
    #[error("snapshot truncated at byte {0}")]
    Truncated(usize),
    #[error("snapshot field `{0}` exceeds its bound")]
    OutOfBounds(&'static str),
    #[error("snapshot string is not valid UTF-8")]
    InvalidUtf8,
    #[error("snapshot has {0} trailing bytes")]
    TrailingBytes(usize),
    #[error("snapshot is internally inconsistent: {0}")]
    Inconsistent(&'static str),
}

#[derive(Debug, Default)]
pub(crate) struct Writer {
    pub(crate) buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub(crate) fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn u128(&mut self, v: u128) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub(crate) fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub(crate) fn len(&mut self, n: usize) {
        self.u64(n as u64);
    }
    pub(crate) fn str(&mut self, s: &str) {
        self.len(s.len());
        self.buf.extend_from_slice(s.as_bytes());
    }
}

#[derive(Debug)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], SnapshotError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or(SnapshotError::Truncated(self.pos))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N], SnapshotError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub(crate) fn expect_magic(&mut self) -> Result<(), SnapshotError> {
        if self.take(SNAPSHOT_MAGIC.len()).ok() == Some(SNAPSHOT_MAGIC.as_slice()) {
            Ok(())
        } else {
            Err(SnapshotError::BadMagic)
        }
    }
    pub(crate) fn u8(&mut self) -> Result<u8, SnapshotError> {
        Ok(self.arr::<1>()?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, SnapshotError> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, SnapshotError> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, SnapshotError> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
    pub(crate) fn u128(&mut self) -> Result<u128, SnapshotError> {
        Ok(u128::from_le_bytes(self.arr()?))
    }
    pub(crate) fn i32(&mut self) -> Result<i32, SnapshotError> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    pub(crate) fn f32(&mut self) -> Result<f32, SnapshotError> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub(crate) fn f64(&mut self) -> Result<f64, SnapshotError> {
        Ok(f64::from_bits(self.u64()?))
    }
    /// Reads a collection length, rejecting values above `max` (and above the global cap).
    pub(crate) fn len(&mut self, field: &'static str, max: u64) -> Result<usize, SnapshotError> {
        let n = self.u64()?;
        if n > max.min(MAX_SNAPSHOT_ITEMS) {
            return Err(SnapshotError::OutOfBounds(field));
        }
        usize::try_from(n).map_err(|_| SnapshotError::OutOfBounds(field))
    }
    pub(crate) fn str(&mut self, field: &'static str) -> Result<String, SnapshotError> {
        let n = self.len(field, MAX_SNAPSHOT_STRING)?;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| SnapshotError::InvalidUtf8)
    }
    pub(crate) fn finish(&self) -> Result<(), SnapshotError> {
        let rest = self.data.len() - self.pos;
        if rest == 0 {
            Ok(())
        } else {
            Err(SnapshotError::TrailingBytes(rest))
        }
    }
}
