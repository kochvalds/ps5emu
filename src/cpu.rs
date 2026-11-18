//! Интерпретатор подмножества x86-64 (ядро Zen 2).
//!
//! Поддерживается: префикс REX, ModRM/SIB, RIP-relative, 8/32/64-битные операнды,
//! флаги CF/ZF/SF/OF. Не поддерживается: 16-битные операнды, SSE/AVX, строковые
//! инструкции, сегменты, FPU, чётность (PF), ADC/SBB, сдвиги, деление.

use std::fmt;

use crate::memory::{MemError, Memory};
use crate::syscall::Kernel;

pub const RAX: usize = 0;
pub const RCX: usize = 1;
pub const RDX: usize = 2;
pub const RBX: usize = 3;
pub const RSP: usize = 4;
pub const RBP: usize = 5;
pub const RSI: usize = 6;
pub const RDI: usize = 7;

#[derive(Debug)]
pub enum CpuError {
    Mem(MemError),
    Unimplemented { rip: u64, opcode: u16 },
}

impl From<MemError> for CpuError {
    fn from(e: MemError) -> Self {
        CpuError::Mem(e)
    }
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuError::Mem(e) => write!(f, "ошибка памяти: {e}"),
            CpuError::Unimplemented { rip, opcode } => {
                write!(f, "инструкция не реализована: опкод {opcode:#x} по адресу {rip:#x}")
            }
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Flags {
    pub cf: bool,
    pub zf: bool,
    pub sf: bool,
    pub of: bool,
}

#[derive(Default, Clone, Copy)]
struct Rex {
    present: bool,
    w: bool,
    r: bool,
    x: bool,
    b: bool,
}

#[derive(Clone, Copy)]
enum Operand {
    Reg(usize),
    Mem(u64),
}

#[derive(Clone, Copy, PartialEq)]
enum Alu {
    Add,
    Or,
    And,
    Sub,
    Xor,
    Cmp,
}

fn alu_from_index(i: u8) -> Option<Alu> {
    match i {
        0 => Some(Alu::Add),
        1 => Some(Alu::Or),
        4 => Some(Alu::And),
        5 => Some(Alu::Sub),
        6 => Some(Alu::Xor),
        7 => Some(Alu::Cmp),
        _ => None,
    }
}

fn mask(size: usize) -> u64 {
    if size >= 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    }
}

fn sign_bit(size: usize) -> u64 {
    1u64 << (size * 8 - 1)
}

fn sign_extend(v: u64, size: usize) -> i64 {
    match size {
        1 => v as u8 as i8 as i64,
        4 => v as u32 as i32 as i64,
        _ => v as i64,
    }
}

#[derive(Default)]
pub struct Cpu {
    pub regs: [u64; 16],
    pub rip: u64,
    pub flags: Flags,
    pub halted: bool,
    pub steps: u64,
}

impl Cpu {
    pub fn new(entry: u64, stack_top: u64) -> Self {
        let mut cpu = Self { rip: entry, ..Default::default() };
        cpu.regs[RSP] = stack_top;
        cpu
    }

    // ---------- выборка ----------

    fn fetch_uint(&mut self, mem: &Memory, size: usize) -> Result<u64, CpuError> {
        let v = mem.read_uint(self.rip, size)?;
        self.rip = self.rip.wrapping_add(size as u64);
        Ok(v)
    }
    fn fetch8(&mut self, mem: &Memory) -> Result<u8, CpuError> {
        Ok(self.fetch_uint(mem, 1)? as u8)
    }
    fn fetch_i8(&mut self, mem: &Memory) -> Result<i64, CpuError> {
        Ok(sign_extend(self.fetch_uint(mem, 1)?, 1))
    }
    fn fetch_i32(&mut self, mem: &Memory) -> Result<i64, CpuError> {
        Ok(sign_extend(self.fetch_uint(mem, 4)?, 4))
    }

    // ---------- регистры и операнды ----------

    fn read_reg(&self, idx: usize, size: usize, rex: Rex) -> u64 {
        match size {
            8 => self.regs[idx],
            4 => self.regs[idx] & 0xFFFF_FFFF,
            _ => {
                if !rex.present && (4..8).contains(&idx) {
                    (self.regs[idx - 4] >> 8) & 0xFF // AH, CH, DH, BH
                } else {
                    self.regs[idx] & 0xFF
                }
            }
        }
    }

    fn write_reg(&mut self, idx: usize, size: usize, val: u64, rex: Rex) {
        match size {
            8 => self.regs[idx] = val,
            4 => self.regs[idx] = val & 0xFFFF_FFFF, // запись в 32 бита обнуляет старшую часть
            _ => {
                if !rex.present && (4..8).contains(&idx) {
                    let r = idx - 4;
                    self.regs[r] = (self.regs[r] & !0xFF00) | ((val & 0xFF) << 8);
                } else {
                    self.regs[idx] = (self.regs[idx] & !0xFF) | (val & 0xFF);
                }
            }
        }
    }

    fn read_op(&self, op: Operand, size: usize, rex: Rex, mem: &Memory) -> Result<u64, CpuError> {
        match op {
            Operand::Reg(i) => Ok(self.read_reg(i, size, rex)),
            Operand::Mem(a) => Ok(mem.read_uint(a, size)?),
        }
    }

    fn write_op(
        &mut self,
        op: Operand,
        size: usize,
        val: u64,
        rex: Rex,
        mem: &mut Memory,
    ) -> Result<(), CpuError> {
        match op {
            Operand::Reg(i) => {
                self.write_reg(i, size, val, rex);
                Ok(())
            }
            Operand::Mem(a) => Ok(mem.write_uint(a, size, val)?),
        }
    }

    /// Разбирает ModRM (+SIB, +смещение). `imm_len` — число байт непосредственного
    /// операнда после ModRM; нужно для RIP-relative адресации.
    fn decode_modrm(
        &mut self,
        mem: &Memory,
        rex: Rex,
        imm_len: u64,
    ) -> Result<(usize, Operand), CpuError> {
        let modrm = self.fetch8(mem)?;
        let mode = modrm >> 6;
        let reg = (((modrm >> 3) & 7) as usize) | ((rex.r as usize) << 3);
        let rm = (modrm & 7) as usize;
        if mode == 3 {
            return Ok((reg, Operand::Reg(rm | ((rex.b as usize) << 3))));
        }
        let addr = if rm == 4 {
            let sib = self.fetch8(mem)?;
            let scale = sib >> 6;
            let index = (((sib >> 3) & 7) as usize) | ((rex.x as usize) << 3);
            let base_low = sib & 7;
            let base = (base_low as usize) | ((rex.b as usize) << 3);
            let no_base = base_low == 5 && mode == 0;
            let mut a = if no_base { 0 } else { self.regs[base] };
            if index != 4 {
                a = a.wrapping_add(self.regs[index] << scale);
            }
            let d = match mode {
                0 => {
                    if no_base {
                        self.fetch_i32(mem)?
                    } else {
                        0
                    }
                }
                1 => self.fetch_i8(mem)?,
                _ => self.fetch_i32(mem)?,
            };
            a.wrapping_add(d as u64)
        } else if rm == 5 && mode == 0 {
            let d = self.fetch_i32(mem)?;
            self.rip.wrapping_add(imm_len).wrapping_add(d as u64)
        } else {
            let a = self.regs[rm | ((rex.b as usize) << 3)];
            let d = match mode {
                0 => 0,
                1 => self.fetch_i8(mem)?,
                _ => self.fetch_i32(mem)?,
            };
            a.wrapping_add(d as u64)
        };
        Ok((reg, Operand::Mem(addr)))
    }

    // ---------- АЛУ и условия ----------

    fn alu(&mut self, op: Alu, a: u64, b: u64, size: usize) -> u64 {
        let m = mask(size);
        let (a, b) = (a & m, b & m);
        let sb = sign_bit(size);
        let res = match op {
            Alu::Add => a.wrapping_add(b) & m,
            Alu::Sub | Alu::Cmp => a.wrapping_sub(b) & m,
            Alu::And => a & b,
            Alu::Or => a | b,
            Alu::Xor => a ^ b,
        };
        match op {
            Alu::Add => {
                self.flags.cf = res < a;
                self.flags.of = (a ^ res) & (b ^ res) & sb != 0;
            }
            Alu::Sub | Alu::Cmp => {
                self.flags.cf = a < b;
                self.flags.of = (a ^ b) & (a ^ res) & sb != 0;
            }
            _ => {
                self.flags.cf = false;
                self.flags.of = false;
            }
        }
        self.flags.zf = res == 0;
        self.flags.sf = res & sb != 0;
        res
    }

    fn cond(&self, cc: u8) -> Option<bool> {
        let f = self.flags;
        Some(match cc {
            0x0 => f.of,
            0x1 => !f.of,
            0x2 => f.cf,
            0x3 => !f.cf,
            0x4 => f.zf,
            0x5 => !f.zf,
            0x6 => f.cf || f.zf,
            0x7 => !f.cf && !f.zf,
            0x8 => f.sf,
            0x9 => !f.sf,
            0xC => f.sf != f.of,
            0xD => f.sf == f.of,
            0xE => f.zf || f.sf != f.of,
            0xF => !f.zf && f.sf == f.of,
            _ => return None, // условия по чётности (PF) не реализованы
        })
    }

    // ---------- стек ----------

    fn push(&mut self, mem: &mut Memory, v: u64) -> Result<(), CpuError> {
        self.regs[RSP] = self.regs[RSP].wrapping_sub(8);
        mem.write_uint(self.regs[RSP], 8, v)?;
        Ok(())
    }

    fn pop(&mut self, mem: &Memory) -> Result<u64, CpuError> {
        let v = mem.read_uint(self.regs[RSP], 8)?;
        self.regs[RSP] = self.regs[RSP].wrapping_add(8);
        Ok(v)
    }

    // ---------- выполнение одной инструкции ----------

    pub fn step(&mut self, mem: &mut Memory, kernel: &mut Kernel) -> Result<(), CpuError> {
        let start = self.rip;
        let bad = |op: u16| CpuError::Unimplemented { rip: start, opcode: op };

        let mut rex = Rex::default();
        let mut opcode = self.fetch8(mem)?;
        if (0x40..=0x4F).contains(&opcode) {
            rex = Rex {
                present: true,
                w: opcode & 8 != 0,
                r: opcode & 4 != 0,
                x: opcode & 2 != 0,
                b: opcode & 1 != 0,
            };
            opcode = self.fetch8(mem)?;
        }
        self.steps += 1;
        let osz: usize = if rex.w { 8 } else { 4 };

        match opcode {
            0x0F => {
                let second = self.fetch8(mem)?;
                match second {
                    0x05 => kernel.syscall(self, mem)?,
                    0x1F => {
                        self.decode_modrm(mem, rex, 0)?; // многобайтовый NOP
                    }
                    0x80..=0x8F => {
                        let rel = self.fetch_i32(mem)?;
                        let taken = self
                            .cond(second & 0xF)
                            .ok_or_else(|| bad(0x0F00 | second as u16))?;
                        if taken {
                            self.rip = self.rip.wrapping_add(rel as u64);
                        }
                    }
                    0xAF => {
                        // IMUL r, r/m
                        let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                        let a = sign_extend(self.read_reg(reg, osz, rex), osz);
                        let b = sign_extend(self.read_op(rm, osz, rex, mem)?, osz);
                        let full = (a as i128) * (b as i128);
                        let res = (a.wrapping_mul(b) as u64) & mask(osz);
                        let overflow = full != sign_extend(res, osz) as i128;
                        self.flags.cf = overflow;
                        self.flags.of = overflow;
                        self.flags.zf = res == 0;
                        self.flags.sf = res & sign_bit(osz) != 0;
                        self.write_reg(reg, osz, res, rex);
                    }
                    0xB6 => {
                        // MOVZX r, r/m8
                        let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                        let v = self.read_op(rm, 1, rex, mem)?;
                        self.write_reg(reg, osz, v, rex);
                    }
                    0xBE => {
                        // MOVSX r, r/m8
                        let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                        let v = sign_extend(self.read_op(rm, 1, rex, mem)?, 1) as u64;
                        self.write_reg(reg, osz, v, rex);
                    }
                    _ => return Err(bad(0x0F00 | second as u16)),
                }
            }
            // ADD/OR/AND/SUB/XOR/CMP во всех стандартных формах
            0x00..=0x3D if (opcode & 7) < 6 => {
                let op = alu_from_index((opcode >> 3) & 7).ok_or_else(|| bad(opcode as u16))?;
                match opcode & 7 {
                    0..=3 => {
                        let size = if opcode & 1 == 0 { 1 } else { osz };
                        let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                        let r = self.read_reg(reg, size, rex);
                        let m = self.read_op(rm, size, rex, mem)?;
                        if opcode & 2 == 0 {
                            let res = self.alu(op, m, r, size);
                            if op != Alu::Cmp {
                                self.write_op(rm, size, res, rex, mem)?;
                            }
                        } else {
                            let res = self.alu(op, r, m, size);
                            if op != Alu::Cmp {
                                self.write_reg(reg, size, res, rex);
                            }
                        }
                    }
                    4 => {
                        let imm = self.fetch_uint(mem, 1)?;
                        let a = self.read_reg(RAX, 1, rex);
                        let res = self.alu(op, a, imm, 1);
                        if op != Alu::Cmp {
                            self.write_reg(RAX, 1, res, rex);
                        }
                    }
                    _ => {
                        let imm = self.fetch_i32(mem)? as u64;
                        let a = self.read_reg(RAX, osz, rex);
                        let res = self.alu(op, a, imm, osz);
                        if op != Alu::Cmp {
                            self.write_reg(RAX, osz, res, rex);
                        }
                    }
                }
            }
            0x50..=0x57 => {
                let reg = ((opcode - 0x50) as usize) | ((rex.b as usize) << 3);
                let v = self.regs[reg];
                self.push(mem, v)?;
            }
            0x58..=0x5F => {
                let reg = ((opcode - 0x58) as usize) | ((rex.b as usize) << 3);
                self.regs[reg] = self.pop(mem)?;
            }
            0x63 => {
                // MOVSXD r, r/m32
                let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                let v = sign_extend(self.read_op(rm, 4, rex, mem)?, 4) as u64;
                self.write_reg(reg, osz, v, rex);
            }
            0x68 => {
                let v = self.fetch_i32(mem)? as u64;
                self.push(mem, v)?;
            }
            0x6A => {
                let v = self.fetch_i8(mem)? as u64;
                self.push(mem, v)?;
            }
            0x70..=0x7F => {
                let rel = self.fetch_i8(mem)?;
                let taken = self.cond(opcode & 0xF).ok_or_else(|| bad(opcode as u16))?;
                if taken {
                    self.rip = self.rip.wrapping_add(rel as u64);
                }
            }
            0x80 | 0x81 | 0x83 => {
                let size = if opcode == 0x80 { 1 } else { osz };
                let imm_len = if opcode == 0x81 { 4 } else { 1 };
                let (regf, rm) = self.decode_modrm(mem, rex, imm_len)?;
                let imm = if opcode == 0x81 { self.fetch_i32(mem)? } else { self.fetch_i8(mem)? } as u64;
                let op = alu_from_index((regf & 7) as u8).ok_or_else(|| bad(opcode as u16))?;
                let a = self.read_op(rm, size, rex, mem)?;
                let res = self.alu(op, a, imm, size);
                if op != Alu::Cmp {
                    self.write_op(rm, size, res, rex, mem)?;
                }
            }
            0x84 | 0x85 => {
                // TEST r/m, r
                let size = if opcode == 0x84 { 1 } else { osz };
                let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                let a = self.read_op(rm, size, rex, mem)?;
                let b = self.read_reg(reg, size, rex);
                self.alu(Alu::And, a, b, size);
            }
            0x88..=0x8B => {
                let size = if opcode & 1 == 0 { 1 } else { osz };
                let (reg, rm) = self.decode_modrm(mem, rex, 0)?;
                if opcode < 0x8A {
                    let v = self.read_reg(reg, size, rex);
                    self.write_op(rm, size, v, rex, mem)?;
                } else {
                    let v = self.read_op(rm, size, rex, mem)?;
                    self.write_reg(reg, size, v, rex);
                }
            }
            0x8D => match self.decode_modrm(mem, rex, 0)? {
                (reg, Operand::Mem(a)) => self.write_reg(reg, osz, a, rex),
                _ => return Err(bad(0x8D)),
            },
            0x90 => {}
            0xB0..=0xB7 => {
                let reg = ((opcode - 0xB0) as usize) | ((rex.b as usize) << 3);
                let imm = self.fetch_uint(mem, 1)?;
                self.write_reg(reg, 1, imm, rex);
            }
            0xB8..=0xBF => {
                let reg = ((opcode - 0xB8) as usize) | ((rex.b as usize) << 3);
                let imm = self.fetch_uint(mem, osz)?;
                self.write_reg(reg, osz, imm, rex);
            }
            0xC3 => self.rip = self.pop(mem)?,
            0xC6 | 0xC7 => {
                let size = if opcode == 0xC6 { 1 } else { osz };
                let imm_len = if opcode == 0xC6 { 1 } else { 4 };
                let (regf, rm) = self.decode_modrm(mem, rex, imm_len)?;
                if regf & 7 != 0 {
                    return Err(bad(opcode as u16));
                }
                let imm = if opcode == 0xC6 {
                    self.fetch_uint(mem, 1)?
                } else {
                    self.fetch_i32(mem)? as u64
                };
                self.write_op(rm, size, imm, rex, mem)?;
            }
            0xE8 => {
                let rel = self.fetch_i32(mem)?;
                let ret = self.rip;
                self.push(mem, ret)?;
                self.rip = self.rip.wrapping_add(rel as u64);
            }
            0xE9 => {
                let rel = self.fetch_i32(mem)?;
                self.rip = self.rip.wrapping_add(rel as u64);
            }
            0xEB => {
                let rel = self.fetch_i8(mem)?;
                self.rip = self.rip.wrapping_add(rel as u64);
            }
            0xF4 => self.halted = true,
            0xFF => {
                let (regf, rm) = self.decode_modrm(mem, rex, 0)?;
                match regf & 7 {
                    0 | 1 => {
                        let v = self.read_op(rm, osz, rex, mem)?;
                        let cf = self.flags.cf; // INC/DEC не меняют CF
                        let op = if regf & 7 == 0 { Alu::Add } else { Alu::Sub };
                        let res = self.alu(op, v, 1, osz);
                        self.flags.cf = cf;
                        self.write_op(rm, osz, res, rex, mem)?;
                    }
                    2 => {
                        let target = self.read_op(rm, 8, rex, mem)?;
                        let ret = self.rip;
                        self.push(mem, ret)?;
                        self.rip = target;
                    }
                    4 => self.rip = self.read_op(rm, 8, rex, mem)?,
                    6 => {
                        let v = self.read_op(rm, 8, rex, mem)?;
                        self.push(mem, v)?;
                    }
                    _ => return Err(bad(0xFF)),
                }
            }
            _ => return Err(bad(opcode as u16)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &[u8], setup: impl FnOnce(&mut Memory)) -> (Cpu, Kernel) {
        let mut mem = Memory::new(1 << 20);
        mem.write_bytes(0x1000, code).unwrap();
        setup(&mut mem);
        let mut cpu = Cpu::new(0x1000, 0xF_F000);
        let mut k = Kernel::new(false);
        for _ in 0..10_000 {
            if cpu.halted {
                break;
            }
            cpu.step(&mut mem, &mut k).unwrap();
        }
        assert!(cpu.halted, "программа не завершилась");
        (cpu, k)
    }

    #[test]
    fn mov_and_add() {
        // mov eax,5 ; add eax,3 ; hlt
        let (cpu, _) = run(&[0xB8, 5, 0, 0, 0, 0x83, 0xC0, 3, 0xF4], |_| {});
        assert_eq!(cpu.regs[RAX], 8);
    }

    #[test]
    fn loop_sums_1_to_10() {
        // xor eax,eax ; mov ecx,10 ; L: add eax,ecx ; dec ecx ; jnz L ; hlt
        let code = [0x31, 0xC0, 0xB9, 10, 0, 0, 0, 0x01, 0xC8, 0xFF, 0xC9, 0x75, 0xFA, 0xF4];
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RAX], 55);
    }

    #[test]
    fn call_and_ret() {
        // call f ; hlt ; f: mov eax,42 ; ret
        let code = [0xE8, 1, 0, 0, 0, 0xF4, 0xB8, 42, 0, 0, 0, 0xC3];
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RAX], 42);
        assert_eq!(cpu.regs[RSP], 0xF_F000); // стек сбалансирован
    }

    #[test]
    fn push_pop() {
        // push 7 ; pop rbx ; hlt
        let (cpu, _) = run(&[0x6A, 7, 0x5B, 0xF4], |_| {});
        assert_eq!(cpu.regs[RBX], 7);
    }

    #[test]
    fn memory_store_and_load_64bit() {
        let mut code = vec![0x48, 0xB8];
        code.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes()); // mov rax, imm64
        code.extend_from_slice(&[0xBB, 0x00, 0x20, 0, 0]); // mov ebx, 0x2000
        code.extend_from_slice(&[0x48, 0x89, 0x03]); // mov [rbx], rax
        code.extend_from_slice(&[0x48, 0x8B, 0x0B]); // mov rcx, [rbx]
        code.push(0xF4);
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RCX], 0x1122_3344_5566_7788);
    }

    #[test]
    fn lea_with_sib() {
        // mov ebx,0x100 ; mov ecx,2 ; lea rax,[rbx+rcx*4+8] ; hlt
        let code = [
            0xBB, 0x00, 0x01, 0, 0, 0xB9, 2, 0, 0, 0, 0x48, 0x8D, 0x44, 0x8B, 0x08, 0xF4,
        ];
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RAX], 0x110);
    }

    #[test]
    fn lea_rip_relative() {
        // lea rax,[rip+0] ; hlt  -> адрес следующей инструкции
        let (cpu, _) = run(&[0x48, 0x8D, 0x05, 0, 0, 0, 0, 0xF4], |_| {});
        assert_eq!(cpu.regs[RAX], 0x1007);
    }

    #[test]
    fn signed_compare_and_jump() {
        // mov eax,3 ; cmp eax,5 ; jl skip ; mov ebx,1 ; skip: hlt
        let code = [0xB8, 3, 0, 0, 0, 0x83, 0xF8, 5, 0x7C, 5, 0xBB, 1, 0, 0, 0, 0xF4];
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RBX], 0);
    }

    #[test]
    fn imul_works() {
        // mov eax,6 ; mov ecx,7 ; imul eax,ecx ; hlt
        let code = [0xB8, 6, 0, 0, 0, 0xB9, 7, 0, 0, 0, 0x0F, 0xAF, 0xC1, 0xF4];
        let (cpu, _) = run(&code, |_| {});
        assert_eq!(cpu.regs[RAX], 42);
    }

    #[test]
    fn write_and_exit_syscalls() {
        let code = [
            0xB8, 4, 0, 0, 0, // mov eax, SYS_write
            0xBF, 1, 0, 0, 0, // mov edi, 1
            0xBE, 0x00, 0x20, 0, 0, // mov esi, 0x2000
            0xBA, 3, 0, 0, 0, // mov edx, 3
            0x0F, 0x05, // syscall
            0xB8, 1, 0, 0, 0, // mov eax, SYS_exit
            0x31, 0xFF, // xor edi, edi
            0x0F, 0x05, // syscall
        ];
        let (_, k) = run(&code, |m| m.write_bytes(0x2000, b"Hi\n").unwrap());
        assert_eq!(k.stdout, b"Hi\n");
        assert_eq!(k.exit_code, Some(0));
    }

    #[test]
    fn unknown_opcode_is_an_error() {
        let mut mem = Memory::new(0x2000);
        mem.write_bytes(0x1000, &[0x06]).unwrap();
        let mut cpu = Cpu::new(0x1000, 0x1F00);
        let mut k = Kernel::new(false);
        assert!(matches!(
            cpu.step(&mut mem, &mut k),
            Err(CpuError::Unimplemented { opcode: 0x06, .. })
        ));
    }
}
