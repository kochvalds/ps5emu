mod cpu;
mod gpu;
#[cfg(feature = "gui")]
mod icon;
mod loader;
mod machine;
mod memory;
mod syscall;
#[cfg(feature = "gui")]
mod ui;

const MAX_STEPS: u64 = 500_000_000;

const HELP: &str = "ps5emu — экспериментальный эмулятор (каркас)\n\n\
Использование:\n  ps5emu                      открыть окно-лаунчер\n  ps5emu --demo               встроенная демо-программа в консоли\n  ps5emu [--trace] файл.elf   запустить ELF в консоли\n\n\
--trace  печатать адрес каждой инструкции в stderr";

#[cfg(feature = "gui")]
fn launch_gui() {
    if let Err(e) = ui::run() {
        eprintln!("Не удалось открыть окно: {e}\nМожно запустить без окна: ps5emu --demo");
        std::process::exit(1);
    }
}

#[cfg(not(feature = "gui"))]
fn launch_gui() {
    println!("{HELP}");
}

fn main() {
    let mut trace = false;
    let mut demo = false;
    let mut path = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--trace" => trace = true,
            "--demo" => demo = true,
            "-h" | "--help" => {
                println!("{HELP}");
                return;
            }
            _ => path = Some(arg),
        }
    }

    let result = match path {
        Some(p) => match std::fs::read(&p) {
            Ok(image) => machine::run_elf(&image, MAX_STEPS, true, trace),
            Err(e) => Err(format!("не удалось прочитать {p}: {e}")),
        },
        None if demo => Ok(machine::run_demo(MAX_STEPS, true)),
        None => {
            launch_gui();
            return;
        }
    };

    match result {
        Ok(r) => {
            if let Some(e) = &r.error {
                eprintln!("Ошибка эмуляции: {e}");
            }
            std::process::exit(r.exit_code);
        }
        Err(e) => {
            eprintln!("Ошибка загрузки: {e}");
            std::process::exit(1);
        }
    }
}
