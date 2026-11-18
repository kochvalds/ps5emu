//! Плоская модель памяти гостя с типизированным доступом (little-endian).

use std::fmt;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemError {
    OutOfBounds(u64),
}

impl fmt::Display for MemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemError::OutOfBounds(a) => write!(f, "доступ за границами памяти по адресу {a:#x}"),
        }
    }
}

pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Self { data: vec![0; size] }
    }

    pub fn size(&self) -> u64 {
        self.data.len() as u64
    }

    fn range(&self, addr: u64, len: usize) -> Result<Range<usize>, MemError> {
        let start = usize::try_from(addr).map_err(|_| MemError::OutOfBounds(addr))?;
        let end = start.checked_add(len).ok_or(MemError::OutOfBounds(addr))?;
        if end > self.data.len() {
            Err(MemError::OutOfBounds(addr))
        } else {
            Ok(start..end)
        }
    }

    pub fn read_bytes(&self, addr: u64, len: usize) -> Result<&[u8], MemError> {
        let r = self.range(addr, len)?;
        Ok(&self.data[r])
    }

    pub fn write_bytes(&mut self, addr: u64, bytes: &[u8]) -> Result<(), MemError> {
        let r = self.range(addr, bytes.len())?;
        self.data[r].copy_from_slice(bytes);
        Ok(())
    }

    /// Читает беззнаковое число размером 1..=8 байт.
    pub fn read_uint(&self, addr: u64, size: usize) -> Result<u64, MemError> {
        let b = self.read_bytes(addr, size)?;
        let mut buf = [0u8; 8];
        buf[..size].copy_from_slice(b);
        Ok(u64::from_le_bytes(buf))
    }

    /// Записывает младшие `size` байт значения.
    pub fn write_uint(&mut self, addr: u64, size: usize, val: u64) -> Result<(), MemError> {
        self.write_bytes(addr, &val.to_le_bytes()[..size])
    }

    pub fn read_u8(&self, addr: u64) -> Result<u8, MemError> {
        Ok(self.read_uint(addr, 1)? as u8)
    }
    pub fn write_u8(&mut self, addr: u64, v: u8) -> Result<(), MemError> {
        self.write_uint(addr, 1, v as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_access_roundtrip() {
        let mut m = Memory::new(64);
        m.write_uint(8, 8, 0x1122_3344_5566_7788).unwrap();
        assert_eq!(m.read_uint(8, 8).unwrap(), 0x1122_3344_5566_7788);
        assert_eq!(m.read_u8(8).unwrap(), 0x88); // little-endian
        assert_eq!(m.read_uint(8, 4).unwrap(), 0x5566_7788);
    }

    #[test]
    fn bounds_are_checked() {
        let mut m = Memory::new(16);
        assert!(m.read_uint(12, 8).is_err());
        assert!(m.write_bytes(u64::MAX, &[1]).is_err());
    }
}
