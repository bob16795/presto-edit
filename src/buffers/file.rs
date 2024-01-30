use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use crate::math::*;
use std::fs::read_to_string;
use std::io::Write;

#[derive(PartialEq, Clone)]
pub enum FileMode {
    Normal,
    Insert,
}

#[derive(Clone)]
pub struct FileBuffer {
    pub filename: String,
    pub cached: bool,
    pub data: Vec<String>,
    pub pos: Vector,
    pub scroll: i32,
    pub mode: FileMode,
    pub height: i32,
    pub char_size: Vector,
}

impl BufferFuncs for FileBuffer {
    fn setup(&mut self, base: &mut Buffer) {
        base.set_var(
            "filetype".to_string(),
            self.filename
                .split('/')
                .last()
                .unwrap()
                .split('.')
                .last()
                .unwrap()
                .to_string(),
        );
    }

    fn update(&mut self, size: Vector) {
        if !self.cached {
            let file = read_to_string(&self.filename);
            if file.is_err() {
                self.data.push("".to_string());
            } else {
                for line in file.unwrap().lines() {
                    self.data.push(line.to_string())
                }
            }
            self.cached = true;
        }

        if size.x < 4 {
            return;
        }

        self.pos.x = self.pos.x.clamp(0, size.x - 6);
        self.pos.y = self.pos.y.clamp(0, self.data.len() as i32 - 1);

        while self.pos.y - self.scroll < 1 && self.scroll > 0 {
            self.scroll -= 1;
        }
        while self.pos.y - self.scroll > self.height - 1 && self.scroll < self.data.len() as i32 {
            self.scroll += 1;
        }
        if self.pos.y < self.data.len() as i32 {
            self.pos.x = self
                .pos
                .x
                .clamp(0, self.data[self.pos.y as usize].len() as i32)
        }
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();

        for idx in 0..coords.h {
            let line_idx = idx + self.scroll;

            if line_idx as usize >= self.data.len() {
                lines.push(drawer::Line::Text {
                    chars: format!(" "),
                    colors: vec![highlight::Color::Link("lineNumberFg".to_string())],
                });
                continue;
            }

            let l = &self.data[line_idx as usize];
            let line = format!("{:>4} {}", line_idx + 1, l);
            let mut colors = Vec::new();

            for _ in 0..5 {
                colors.push(highlight::Color::Link("lineNumberFg".to_string()));
            }

            for ch in l.chars() {
                if ch.is_numeric() {
                    colors.push(highlight::Color::Link("fg".to_string()));
                } else {
                    colors.push(highlight::Color::Link("fg".to_string()));
                }
            }

            lines.push(drawer::Line::Text {
                chars: line,
                colors,
            });
        }

        let w = handle.get_char_size()?.x;

        handle.render_rect(
            Vector {
                x: coords.x,
                y: coords.y,
            },
            Vector {
                x: (w as f32 * 4.5) as i32,
                y: coords.h,
            },
            highlight::Color::Link("lineNumberBg".to_string()),
        )?;

        handle.render_line(
            Vector {
                x: coords.x + (w as f32 * 4.5) as i32,
                y: coords.y,
            },
            Vector {
                x: coords.x + (w as f32 * 4.5) as i32,
                y: coords.y + coords.h,
            },
            highlight::Color::Link("lineNumberSplit".to_string()),
        )?;

        handle.render_text(lines, coords, drawer::TextMode::Lines)?;

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData {
        self.height = size.y / char_size.y;

        self.char_size = char_size;

        let mut result = drawer::CursorData::Show {
            pos: Vector {
                x: self.pos.x * char_size.x,
                y: self.pos.y * char_size.y,
            },
            size: char_size,
            kind: if self.mode == FileMode::Normal {
                drawer::CursorStyle::Block
            } else {
                drawer::CursorStyle::Bar
            },
        };
        result.offset(Vector {
            x: 5 * char_size.x,
            y: -self.scroll * char_size.y,
        });

        result
    }

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
        let targ_none = event::Mods {
            ctrl: false,
            alt: false,
            shift: false,
        };
        //let targ_ctrl = event::Mods {
        //    ctrl: true,
        //    alt: false,
        //    shift: false,
        //};

        match (self.mode.clone(), ev) {
            (_, event::Event::Nav(mods, event::Nav::Down)) if mods == targ_none => {
                self.pos.y += 1;
                return;
            }
            (_, event::Event::Nav(mods, event::Nav::Up)) if mods == targ_none => {
                self.pos.y -= 1;
                return;
            }
            (_, event::Event::Nav(mods, event::Nav::Left)) if mods == targ_none => {
                self.pos.x -= 1;
                return;
            }
            (_, event::Event::Nav(mods, event::Nav::Right)) if mods == targ_none => {
                self.pos.x += 1;
                return;
            }
            (FileMode::Insert, event::Event::Nav(mods, event::Nav::Enter)) if mods == targ_none => {
                let next = self.data[self.pos.y as usize].split_off(self.pos.x as usize);
                self.data.insert((self.pos.y + 1) as usize, next);
                self.pos.x = 0;
                self.pos.y += 1;

                return;
            }
            (FileMode::Insert, event::Event::Nav(mods, event::Nav::BackSpace))
                if mods == targ_none =>
            {
                if self.pos.x > 0 {
                    self.data[self.pos.y as usize].remove((self.pos.x - 1) as usize);
                    self.pos.x -= 1;
                } else if self.pos.y > 0 {
                    self.pos.x = self.data[(self.pos.y - 1) as usize].len() as i32;
                    let adds = self.data[self.pos.y as usize].clone();
                    self.data[(self.pos.y - 1) as usize].push_str(&adds);
                    self.data.remove(self.pos.y as usize);
                    self.pos.y -= 1;
                }

                return;
            }
            (FileMode::Insert, event::Event::Nav(mods, event::Nav::Escape))
                if mods == targ_none =>
            {
                self.mode = FileMode::Normal;
            }
            (_, event::Event::Save(None)) => {
                let mut file = std::fs::File::create(self.filename.as_str()).unwrap();
                let mut conts: String = "".to_string();
                for line in &self.data {
                    let _ = file.write(line.as_bytes());
                    let _ = file.write(b"\n");
                    conts += line;
                    conts.push('\n');
                }

                lsp.save_file(self.filename.clone(), conts).unwrap();
            }
            (FileMode::Insert, event::Event::Key(mods, c)) if mods == targ_none => {
                self.data[self.pos.y as usize].insert(self.pos.x as usize, c);
                self.pos.x += 1;
                return;
            }
            (FileMode::Normal, event::Event::Key(mods, c)) if mods == targ_none && c == 'i' => {
                self.mode = FileMode::Insert;
            }
            (_, event::Event::Mouse(pos, _btn)) => {
                self.pos.x = (pos.x - coords.x) / self.char_size.x - 5;
                self.pos.y = (pos.y - coords.y) / self.char_size.y + self.scroll;
            }
            _ => {}
        }
    }

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("File[{}]", self.filename)
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        false
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        lsp.close_file(self.filename.clone()).unwrap();
        CloseKind::This
    }
}
