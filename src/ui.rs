//! Лаунчер в стиле консольного дашборда: тёмный фон со свечением, вкладки сверху,
//! горизонтальная лента плиток, управление стрелками/Enter/Esc.
//! Файлы .elf можно перетащить в окно — они появятся в ленте.

use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Rect, Sense, Shape, Stroke};

use crate::machine::{self, RunResult};

const GUI_MAX_STEPS: u64 = 50_000_000;

#[derive(Clone)]
enum Kind {
    Demo,
    Open,
    About,
    File(Vec<u8>),
}

struct Entry {
    title: String,
    subtitle: String,
    color: Color32,
    kind: Kind,
}

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    Home,
    Running,
    About,
}

pub struct Launcher {
    entries: Vec<Entry>,
    selected: usize,
    tab: usize,
    screen: Screen,
    rx: Option<Receiver<RunResult>>,
    result: Option<RunResult>,
    running_title: String,
    toast: String,
    toast_until: f64,
    now: f64,
}

impl Launcher {
    pub fn new() -> Self {
        let entries = vec![
            Entry {
                title: "Hello ps5emu".into(),
                subtitle: "Демо-программа: syscall write + exit".into(),
                color: Color32::from_rgb(30, 90, 200),
                kind: Kind::Demo,
            },
            Entry {
                title: "Открыть ELF".into(),
                subtitle: "Перетащите файл .elf в окно".into(),
                color: Color32::from_rgb(20, 120, 110),
                kind: Kind::Open,
            },
            Entry {
                title: "О программе".into(),
                subtitle: "Что умеет и чего не умеет ps5emu".into(),
                color: Color32::from_rgb(110, 50, 170),
                kind: Kind::About,
            },
        ];
        Self {
            entries,
            selected: 0,
            tab: 0,
            screen: Screen::Home,
            rx: None,
            result: None,
            running_title: String::new(),
            toast: String::new(),
            toast_until: 0.0,
            now: 0.0,
        }
    }

    fn show_toast(&mut self, text: String) {
        self.toast = text;
        self.toast_until = self.now + 3.0;
    }

    fn launch(&mut self, title: String, image: Option<Vec<u8>>) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.result = None;
        self.running_title = title;
        self.screen = Screen::Running;
        std::thread::spawn(move || {
            let r = match image {
                None => machine::run_demo(GUI_MAX_STEPS, false),
                Some(img) => match machine::run_elf(&img, GUI_MAX_STEPS, false, false) {
                    Ok(r) => r,
                    Err(e) => RunResult { stdout: vec![], exit_code: 1, error: Some(e), steps: 0 },
                },
            };
            let _ = tx.send(r);
        });
    }

    fn activate(&mut self) {
        let title = self.entries[self.selected].title.clone();
        let kind = self.entries[self.selected].kind.clone();
        match kind {
            Kind::Demo => self.launch(title, None),
            Kind::File(bytes) => self.launch(title, Some(bytes)),
            Kind::Open => self.show_toast("Перетащите файл .elf в окно эмулятора".into()),
            Kind::About => self.screen = Screen::About,
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let name = match &f.path {
                Some(p) => p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.name.clone()),
                None => f.name.clone(),
            };
            let bytes = if let Some(b) = &f.bytes {
                Some(b.to_vec())
            } else if let Some(p) = &f.path {
                std::fs::read(p).ok()
            } else {
                None
            };
            if let Some(bytes) = bytes {
                let h = name
                    .bytes()
                    .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
                let color = Color32::from_rgb(
                    40 + (h % 100) as u8,
                    60 + ((h >> 8) % 100) as u8,
                    140 + ((h >> 16) % 100) as u8,
                );
                self.entries.insert(
                    1,
                    Entry { title: name.clone(), subtitle: "Пользовательский ELF".into(), color, kind: Kind::File(bytes) },
                );
                self.selected = 1;
                self.screen = Screen::Home;
                self.tab = 0;
                self.show_toast(format!("Добавлено: {name}"));
            }
        }
    }

    fn poll(&mut self) {
        let got = self.rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(r) = got {
            self.result = Some(r);
            self.rx = None;
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let (left, right, enter, esc, tab) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Tab),
            )
        });
        match self.screen {
            Screen::Home => {
                if left && self.selected > 0 {
                    self.selected -= 1;
                }
                if right && self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
                if tab {
                    self.tab = 1 - self.tab;
                }
                if enter && self.tab == 0 {
                    self.activate();
                }
            }
            Screen::Running | Screen::About => {
                if esc || enter {
                    self.screen = Screen::Home;
                }
            }
        }
    }

    // ---------- рисование ----------

    fn paint_background(&self, ui: &egui::Ui, rect: Rect, t: f64) {
        let p = ui.painter().clone();
        let top = Color32::from_rgb(8, 14, 40);
        let bottom = Color32::from_rgb(2, 4, 14);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.left_top(), top);
        mesh.colored_vertex(rect.right_top(), top);
        mesh.colored_vertex(rect.right_bottom(), bottom);
        mesh.colored_vertex(rect.left_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(Shape::mesh(mesh));

        for k in 0..3 {
            let ph = t as f32 * 0.15 + k as f32 * 2.1;
            let c = pos2(
                rect.center().x + ph.sin() * rect.width() * 0.3,
                rect.center().y * 0.8 + (ph * 1.3).cos() * rect.height() * 0.15,
            );
            for r in 0..8 {
                let rad = 120.0 + r as f32 * 45.0;
                p.circle_filled(c, rad, Color32::from_rgba_unmultiplied(40, 90, 255, 6));
            }
        }
    }

    fn paint_topbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        let y = rect.top() + 56.0;
        for (i, label) in ["Игры", "Медиа"].iter().enumerate() {
            let x = rect.center().x + if i == 0 { -80.0 } else { 80.0 };
            let active = self.tab == i;
            let (size, col) = if active {
                (28.0, Color32::WHITE)
            } else {
                (21.0, Color32::from_white_alpha(120))
            };
            p.text(pos2(x, y), Align2::CENTER_CENTER, *label, FontId::proportional(size), col);
            if active {
                p.rect_filled(Rect::from_center_size(pos2(x, y + 24.0), vec2(36.0, 4.0)), 2.0, Color32::WHITE);
            }
            let r = Rect::from_center_size(pos2(x, y), vec2(130.0, 50.0));
            if ui.interact(r, egui::Id::new(("tab", i)), Sense::click()).clicked() {
                self.tab = i;
            }
        }
        p.text(
            pos2(rect.right() - 28.0, rect.top() + 34.0),
            Align2::RIGHT_CENTER,
            format!("ps5emu v{}", env!("CARGO_PKG_VERSION")),
            FontId::proportional(16.0),
            Color32::from_white_alpha(110),
        );
    }

    fn paint_tiles(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        let scroll = ui
            .ctx()
            .animate_value_with_time(egui::Id::new("scroll"), self.selected as f32, 0.25);
        let cy = rect.center().y + 10.0;
        let spacing = 250.0;
        let mut clicked_selected = false;
        let mut new_selected = None;

        for (i, e) in self.entries.iter().enumerate() {
            let d = i as f32 - scroll;
            let x = rect.center().x + d * spacing;
            if x < rect.left() - 200.0 || x > rect.right() + 200.0 {
                continue;
            }
            let sel = (1.0 - d.abs()).clamp(0.0, 1.0);
            let size = 190.0 + 60.0 * sel;
            let tile = Rect::from_center_size(pos2(x, cy), vec2(size, size));

            p.rect_filled(tile, 22.0, e.color);
            let lower = Rect::from_min_max(pos2(tile.left(), tile.center().y), tile.right_bottom());
            p.rect_filled(
                lower,
                egui::Rounding { nw: 0.0, ne: 0.0, sw: 22.0, se: 22.0 },
                Color32::from_black_alpha(70),
            );
            for g in 1..=4 {
                let gf = g as f32;
                p.rect_stroke(
                    tile.expand(gf * 3.0),
                    22.0 + gf * 3.0,
                    Stroke::new(3.0, Color32::from_white_alpha((70.0 * sel / gf) as u8)),
                );
            }
            p.rect_stroke(tile, 22.0, Stroke::new(3.0, Color32::from_white_alpha((255.0 * sel) as u8)));

            // глиф плитки
            let c = tile.center();
            match &e.kind {
                Kind::Demo => {
                    p.add(Shape::convex_polygon(
                        vec![pos2(c.x - 18.0, c.y - 28.0), pos2(c.x - 18.0, c.y + 28.0), pos2(c.x + 30.0, c.y)],
                        Color32::WHITE,
                        Stroke::NONE,
                    ));
                }
                Kind::Open => {
                    p.text(c, Align2::CENTER_CENTER, "+", FontId::proportional(80.0), Color32::WHITE);
                }
                Kind::About => {
                    p.text(c, Align2::CENTER_CENTER, "i", FontId::proportional(80.0), Color32::WHITE);
                }
                Kind::File(_) => {
                    p.text(c, Align2::CENTER_CENTER, "ELF", FontId::proportional(44.0), Color32::WHITE);
                }
            }

            p.text(
                pos2(x, tile.bottom() + 28.0),
                Align2::CENTER_CENTER,
                &e.title,
                FontId::proportional(18.0 + 6.0 * sel),
                Color32::from_white_alpha((150.0 + 105.0 * sel) as u8),
            );
            if sel > 0.5 {
                p.text(
                    pos2(x, tile.bottom() + 56.0),
                    Align2::CENTER_CENTER,
                    &e.subtitle,
                    FontId::proportional(15.0),
                    Color32::from_white_alpha((200.0 * sel) as u8),
                );
            }

            if ui.interact(tile, egui::Id::new(("tile", i)), Sense::click()).clicked() {
                if self.selected == i {
                    clicked_selected = true;
                } else {
                    new_selected = Some(i);
                }
            }
        }
        if let Some(i) = new_selected {
            self.selected = i;
        }
        if clicked_selected {
            self.activate();
        }
    }

    fn paint_media(&self, ui: &egui::Ui, rect: Rect) {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Медиатека появится позже",
            FontId::proportional(26.0),
            Color32::from_white_alpha(140),
        );
    }

    fn paint_console(&self, ui: &egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        let panel = Rect::from_center_size(
            rect.center() + vec2(0.0, 20.0),
            vec2(rect.width() * 0.8, rect.height() * 0.62),
        );
        p.rect_filled(panel, 16.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));
        p.rect_stroke(panel, 16.0, Stroke::new(1.5, Color32::from_white_alpha(50)));
        let x = panel.left() + 28.0;
        let mut y = panel.top() + 24.0;
        let head = p.text(
            pos2(x, y),
            Align2::LEFT_TOP,
            &self.running_title,
            FontId::proportional(24.0),
            Color32::WHITE,
        );
        y = head.bottom() + 18.0;
        match &self.result {
            None => {
                p.text(pos2(x, y), Align2::LEFT_TOP, "Выполняется…", FontId::proportional(18.0), Color32::from_white_alpha(160));
            }
            Some(r) => {
                let out = String::from_utf8_lossy(&r.stdout).to_string();
                if !out.is_empty() {
                    let o = p.text(pos2(x, y), Align2::LEFT_TOP, out, FontId::monospace(18.0), Color32::from_rgb(140, 255, 170));
                    y = o.bottom() + 12.0;
                }
                if let Some(e) = &r.error {
                    let o = p.text(
                        pos2(x, y),
                        Align2::LEFT_TOP,
                        format!("Ошибка: {e}"),
                        FontId::proportional(17.0),
                        Color32::from_rgb(255, 120, 120),
                    );
                    y = o.bottom() + 12.0;
                }
                p.text(
                    pos2(x, y),
                    Align2::LEFT_TOP,
                    format!("Код завершения: {} · выполнено инструкций: {}", r.exit_code, r.steps),
                    FontId::proportional(15.0),
                    Color32::from_white_alpha(130),
                );
            }
        }
    }

    fn paint_about(&self, ui: &egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        let panel = Rect::from_center_size(rect.center() + vec2(0.0, 20.0), vec2(rect.width() * 0.7, rect.height() * 0.6));
        p.rect_filled(panel, 16.0, Color32::from_rgba_unmultiplied(0, 0, 0, 190));
        p.rect_stroke(panel, 16.0, Stroke::new(1.5, Color32::from_white_alpha(50)));
        let text = format!(
            "ps5emu v{}\n\n\
             Учебный каркас эмулятора: интерпретатор подмножества x86-64,\n\
             загрузчик ELF и минимальные системные вызовы в стиле FreeBSD.\n\n\
             Это НЕ рабочий эмулятор PS5. Коммерческие игры\n\
             (включая GTA 6) он запускать не умеет.\n\n\
             Прошивка, ключи и дампы игр не нужны и не поддерживаются.",
            env!("CARGO_PKG_VERSION")
        );
        p.text(
            panel.left_top() + vec2(32.0, 28.0),
            Align2::LEFT_TOP,
            text,
            FontId::proportional(19.0),
            Color32::from_white_alpha(225),
        );
    }

    fn paint_hint(&self, ui: &egui::Ui, rect: Rect) {
        let hint = match self.screen {
            Screen::Home => "←  →  выбор      Enter  запуск      Tab  вкладка      Перетащите .elf в окно",
            Screen::Running | Screen::About => "Esc / Enter  назад",
        };
        ui.painter().text(
            pos2(rect.center().x, rect.bottom() - 30.0),
            Align2::CENTER_CENTER,
            hint,
            FontId::proportional(16.0),
            Color32::from_white_alpha(110),
        );
        if self.now < self.toast_until {
            let c = pos2(rect.center().x, rect.bottom() - 80.0);
            let r = Rect::from_center_size(c, vec2(420.0, 40.0));
            ui.painter().rect_filled(r, 20.0, Color32::from_white_alpha(230));
            ui.painter().text(c, Align2::CENTER_CENTER, &self.toast, FontId::proportional(17.0), Color32::from_rgb(10, 20, 50));
        }
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.now = ctx.input(|i| i.time);
        self.handle_drops(ctx);
        self.poll();
        self.handle_keys(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let t = self.now;
                self.paint_background(ui, rect, t);
                match self.screen {
                    Screen::Home => {
                        self.paint_topbar(ui, rect);
                        if self.tab == 0 {
                            self.paint_tiles(ui, rect);
                        } else {
                            self.paint_media(ui, rect);
                        }
                    }
                    Screen::Running => self.paint_console(ui, rect),
                    Screen::About => self.paint_about(ui, rect),
                }
                self.paint_hint(ui, rect);
            });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

pub fn run() -> Result<(), eframe::Error> {
    let icon = egui::IconData { rgba: crate::icon::rgba(256), width: 256, height: 256 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("ps5emu")
            .with_app_id("ps5emu")
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native("ps5emu", options, Box::new(|_cc| Ok(Box::new(Launcher::new()))))
}
