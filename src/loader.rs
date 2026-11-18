//! Минимальный загрузчик ELF64 (x86-64). Грузит только сегменты PT_LOAD.
//! TODO: релокации, импорты библиотек, динамическая линковка, права доступа страниц.

use crate::memory::Memory;

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}
fn u64_at(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

/// Загружает ELF в память и возвращает точку входа.
pub fn load_elf(image: &[u8], mem: &mut Memory) -> Result<u64, String> {
    if image.get(0..4) != Some(&[0x7F, b'E', b'L', b'F'][..]) {
        return Err("не ELF-файл".into());
    }
    if image.get(4) != Some(&2) {
        return Err("нужен ELF64".into());
    }
    if u16_at(image, 18) != Some(0x3E) {
        return Err("нужна архитектура x86-64".into());
    }
    let bad = || "повреждённый заголовок".to_string();
    let entry = u64_at(image, 24).ok_or_else(bad)?;
    let phoff = u64_at(image, 32).ok_or_else(bad)? as usize;
    let phentsize = u16_at(image, 54).ok_or_else(bad)? as usize;
    let phnum = u16_at(image, 56).ok_or_else(bad)? as usize;

    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        let p_type = u32_at(image, ph).ok_or_else(bad)?;
        if p_type != 1 {
            continue; // не PT_LOAD
        }
        let offset = u64_at(image, ph + 8).ok_or_else(bad)? as usize;
        let vaddr = u64_at(image, ph + 16).ok_or_else(bad)?;
        let filesz = u64_at(image, ph + 32).ok_or_else(bad)? as usize;
        let end = offset.checked_add(filesz).ok_or_else(bad)?;
        let seg = image.get(offset..end).ok_or_else(bad)?;
        mem.write_bytes(vaddr, seg).map_err(|e| e.to_string())?;
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_elf() -> Vec<u8> {
        let mut img = vec![0u8; 121];
        img[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        img[4] = 2;
        img[18] = 0x3E;
        img[24..32].copy_from_slice(&0x1000u64.to_le_bytes()); // e_entry
        img[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
        img[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        img[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
        img[64..68].copy_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
        img[72..80].copy_from_slice(&120u64.to_le_bytes()); // p_offset
        img[80..88].copy_from_slice(&0x1000u64.to_le_bytes()); // p_vaddr
        img[96..104].copy_from_slice(&1u64.to_le_bytes()); // p_filesz
        img[120] = 0xF4; // hlt
        img
    }

    #[test]
    fn loads_minimal_elf() {
        let mut mem = Memory::new(1 << 16);
        let entry = load_elf(&minimal_elf(), &mut mem).unwrap();
        assert_eq!(entry, 0x1000);
        assert_eq!(mem.read_u8(0x1000).unwrap(), 0xF4);
    }

    #[test]
    fn rejects_garbage() {
        let mut mem = Memory::new(1 << 16);
        assert!(load_elf(b"hello", &mut mem).is_err());
    }
}
