//! Минимальное «ядро»: системные вызовы в стиле FreeBSD (ОС PS5 основана на ней).
//! Конвенция: номер в RAX, аргументы RDI, RSI, RDX, R10, R8, R9;
//! результат в RAX, при ошибке выставляется CF и в RAX лежит errno.
//! TODO: файловая система, потоки, mmap, мьютексы, таймеры, сеть.

use std::io::Write;

use crate::cpu::{Cpu, CpuError, RAX, RDI, RDX, RSI};
use crate::memory::Memory;

pub const SYS_EXIT: u64 = 1;
pub const SYS_WRITE: u64 = 4;
pub const SYS_GETPID: u64 = 20;

const EBADF: u64 = 9;
const ENOSYS: u64 = 78;

pub struct Kernel {
    /// Всё, что гость записал в stdout/stderr.
    pub stdout: Vec<u8>,
    pub exit_code: Option<i32>,
    echo: bool,
}

impl Kernel {
    pub fn new(echo: bool) -> Self {
        Self { stdout: Vec::new(), exit_code: None, echo }
    }

    pub fn syscall(&mut self, cpu: &mut Cpu, mem: &mut Memory) -> Result<(), CpuError> {
        cpu.flags.cf = false;
        match cpu.regs[RAX] {
            SYS_EXIT => {
                self.exit_code = Some(cpu.regs[RDI] as i32);
                cpu.halted = true;
            }
            SYS_WRITE => {
                let fd = cpu.regs[RDI];
                let buf = cpu.regs[RSI];
                let len = cpu.regs[RDX] as usize;
                if fd != 1 && fd != 2 {
                    cpu.regs[RAX] = EBADF;
                    cpu.flags.cf = true;
                } else {
                    let bytes = mem.read_bytes(buf, len)?.to_vec();
                    self.stdout.extend_from_slice(&bytes);
                    if self.echo {
                        let _ = std::io::stdout().write_all(&bytes);
                    }
                    cpu.regs[RAX] = len as u64;
                }
            }
            SYS_GETPID => cpu.regs[RAX] = 1,
            _ => {
                cpu.regs[RAX] = ENOSYS;
                cpu.flags.cf = true;
            }
        }
        Ok(())
    }
}
