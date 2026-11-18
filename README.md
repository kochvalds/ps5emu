<p align="center"><img src="assets/ps5emu.png" width="128" alt="ps5emu"></p>

# ps5emu

Экспериментальный каркас эмулятора PS5 на Rust с лаунчером в стиле консольного
дашборда. **Это не рабочий эмулятор PS5**: коммерческие игры (включая GTA 6) он
запускать не умеет. Проект — база для обучения и экспериментов: интерпретатор
подмножества x86-64, загрузчик ELF, минимальное «ядро» с системными вызовами в
стиле FreeBSD и заглушка GPU. Дизайн и иконка оригинальные, без логотипов третьих лиц.

## Скачать (AppImage)
Каждый push в `main` автоматически собирает AppImage и обновляет релиз **latest**
(раздел **Releases** справа на главной странице репозитория). Версионный релиз:
`git tag v0.3.0 && git push --tags`.
```
chmod +x ps5emu-x86_64.AppImage
./ps5emu-x86_64.AppImage
```
Нужны стандартные системные библиотеки рабочего стола (X11 или Wayland, OpenGL).

## Лаунчер
Стрелки ← → выбор плитки, Enter запуск, Tab смена вкладки, Esc назад.
Файл `.elf` можно перетащить в окно — он появится в ленте.

## Сборка из исходников
```
cargo test
cargo run                       # окно-лаунчер
cargo run -- --demo             # демо в консоли
cargo run -- app.elf            # ELF в консоли
cargo run --no-default-features # только CLI, без окна
```
Локальная сборка AppImage: `cargo build --release && bash packaging/build-appimage.sh`.
Пересоздать иконку: `python3 tools/gen_icon.py 256 assets/ps5emu.png`.

## Что реализовано
| Область | Состояние |
|---|---|
| Префикс REX, ModRM/SIB, RIP-relative | да |
| MOV, MOVZX/MOVSX/MOVSXD, LEA, PUSH/POP | да |
| ADD/OR/AND/SUB/XOR/CMP/TEST, INC/DEC, IMUL | да (флаги CF/ZF/SF/OF) |
| CALL/RET/JMP, все Jcc кроме PF | да |
| SYSCALL: exit, write, getpid | да |
| ELF64 (PT_LOAD) | да |
| Лаунчер, framebuffer + экспорт PPM | да |
| 16-битные операнды, сдвиги, деление, ADC/SBB, PF | нет |
| SSE/AVX2, FPU, строковые инструкции | нет |
| MMU, потоки, динамическая линковка | нет |
| GPU (AGC/GNM → Vulkan), аудио, ввод, SSD | нет |

## Структура
`src/cpu.rs` интерпретатор · `src/memory.rs` память · `src/syscall.rs` ядро ·
`src/loader.rs` ELF · `src/machine.rs` сборка консоли · `src/gpu.rs` GPU-заглушка ·
`src/ui.rs` лаунчер · `src/icon.rs` иконка · `packaging/` AppImage · `.github/workflows/` CI

Проект не содержит и не принимает прошивку, ключи или дампы игр.
