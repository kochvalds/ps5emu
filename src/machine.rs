//! Сборка «консоли»: память + CPU + ядро + GPU. Используется и CLI, и окном.

use crate::cpu::Cpu;
use crate::gpu::{Gpu, NullGpu};
use crate::loader;
use crate::memory::Memory;
use crate::syscall::Kernel;

pub const MEM_SIZE: usize = 64 * 1024 * 1024;
pub const STACK_TOP: u64 = MEM_SIZE as u64 - 0x1000;

pub struct RunResult {
    pub stdout: Vec<u8>,
    pub exit_code: i32,
    pub error: Option<String>,
    pub steps: u64,
}

pub fn run(mem: &mut Memory, entry: u64, max_steps: u64, echo: bool, trace: bool) -> RunResult {
    let mut cpu = Cpu::new(entry, STACK_TOP);
    let mut kernel = Kernel::new(echo);
    let mut gpu = NullGpu;
    let mut error = None;
    while !cpu.halted {
        if cpu.steps >= max_steps {
            error = Some(format!("Превышен лимит шагов ({max_steps})"));
            break;
        }
        if trace {
            eprintln!("rip={:#012x}", cpu.rip);
        }
        if let Err(e) = cpu.step(mem, &mut kernel) {
            error = Some(e.to_string());
            break;
        }
    }
    if error.is_none() {
        gpu.present();
    }
    let exit_code = if error.is_some() { 1 } else { kernel.exit_code.unwrap_or(0) };
    RunResult { stdout: kernel.stdout, exit_code, error, steps: cpu.steps }
}

/// Демо: write(1, "Hello from ps5emu!\n", 19); exit(0)
pub fn load_demo(mem: &mut Memory) -> u64 {
    let msg = b"Hello from ps5emu!\n";
    mem.write_bytes(0x2000, msg).unwrap();
    let code = [
        0xB8, 4, 0, 0, 0, //             mov eax, SYS_write
        0xBF, 1, 0, 0, 0, //             mov edi, 1
        0xBE, 0x00, 0x20, 0, 0, //       mov esi, 0x2000
        0xBA, msg.len() as u8, 0, 0, 0, // mov edx, len
        0x0F, 0x05, //                   syscall
        0xB8, 1, 0, 0, 0, //             mov eax, SYS_exit
        0x31, 0xFF, //                   xor edi, edi
        0x0F, 0x05, //                   syscall
    ];
    mem.write_bytes(0x1000, &code).unwrap();
    0x1000
}

pub fn run_demo(max_steps: u64, echo: bool) -> RunResult {
    let mut mem = Memory::new(MEM_SIZE);
    let entry = load_demo(&mut mem);
    run(&mut mem, entry, max_steps, echo, false)
}

pub fn run_elf(image: &[u8], max_steps: u64, echo: bool, trace: bool) -> Result<RunResult, String> {
    let mut mem = Memory::new(MEM_SIZE);
    let entry = loader::load_elf(image, &mut mem)?;
    Ok(run(&mut mem, entry, max_steps, echo, trace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_prints_hello_and_exits_cleanly() {
        let r = run_demo(1_000, false);
        assert_eq!(r.stdout, b"Hello from ps5emu!\n");
        assert_eq!(r.exit_code, 0);
        assert!(r.error.is_none());
    }

    #[test]
    fn step_limit_is_reported() {
        let mut mem = Memory::new(0x4000);
        mem.write_bytes(0x1000, &[0xEB, 0xFE]).unwrap(); // jmp $ (бесконечный цикл)
        let r = run(&mut mem, 0x1000, 100, false, false);
        assert!(r.error.is_some());
        assert_eq!(r.exit_code, 1);
    }
}
