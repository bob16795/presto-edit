use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use std::fs::read;
use std::io::Write;

#[derive(Clone, PartialEq)]
pub enum HexMode {
    Normal,
    Insert,
}

#[derive(Clone)]
pub struct HexBuffer {
    pub filename: String,
    pub cached: bool,
    pub data: Vec<u8>,
    pub pos: Vector,
    pub scroll: i32,
    pub mode: HexMode,
    pub height: i32,
    pub char_size: Vector,
}

impl BufferFuncs for HexBuffer {
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
            let file = read(&self.filename);
            if file.is_err() {
                self.data = vec![0];
            } else {
                self.data = file.unwrap();
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
            self.pos.x = self.pos.x.clamp(0, 15)
        }
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();
        let mut i = 16 * self.scroll as usize;

        for _ in 0..coords.h {
            let mut line = "".to_string();
            let mut suff = "".to_string();
            let mut colors = Vec::new();
            line += format!("{:08X} ", i).as_str();
            colors.extend(vec![highlight::Color::Link("lineNumberFg".to_string()); 9]);

            for _ in 0..4 {
                for _ in 0..4 {
                    if i < self.data.len() {
                        line += format!("{:02X}", self.data[i]).as_str();
                        if self.data[i] == 0 {
                            suff.push(' ');
                            colors.extend(vec![highlight::Color::Link("error".to_string()); 2]);
                        } else if self.data[i] > 32 && self.data[i] < 128 {
                            suff.push(self.data[i] as char);
                            colors.extend(vec![highlight::Color::Link("fg".to_string()); 2]);
                        } else {
                            suff.push('.');
                            colors.extend(vec![highlight::Color::Link("error".to_string()); 2]);
                        }
                        i += 1;
                    } else {
                        line += format!("..").as_str();
                        colors.extend(vec![highlight::Color::Link("fg".to_string()); 2]);
                    }
                }
                line += format!(" ").as_str();
                colors.extend(vec![highlight::Color::Link("fg".to_string()); 1]);
            }

            line += &suff;

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
                x: (w as f32 * 8.5) as i32,
                y: coords.h,
            },
            highlight::Color::Link("lineNumberBg".to_string()),
        )?;

        handle.render_line(
            Vector {
                x: coords.x + (w as f32 * 8.5) as i32,
                y: coords.y,
            },
            Vector {
                x: coords.x + (w as f32 * 8.5) as i32,
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
                x: (self.pos.x * 2 + self.pos.x / 4) * char_size.x,
                y: self.pos.y * char_size.y,
            },
            size: Vector {
                x: char_size.x * 2,
                y: char_size.y,
            },
            kind: if self.mode == HexMode::Normal {
                drawer::CursorStyle::Block
            } else {
                drawer::CursorStyle::Bar
            },
        };

        result.offset(Vector {
            x: 9 * char_size.x,
            y: -self.scroll * char_size.y,
        });

        result
    }

    fn event_process(&mut self, ev: event::Event, _lsp: &mut lsp::LSP, coords: Rect) {
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
            (HexMode::Insert, event::Event::Nav(mods, event::Nav::Escape)) if mods == targ_none => {
                self.mode = HexMode::Normal;
            }
            (_, event::Event::Save(None)) => {
                let mut file = std::fs::File::create(self.filename.as_str()).unwrap();
                let _ = file.write(&self.data);
            }
            //(HexMode::Insert, event::Event::Key(mods, c)) if mods == targ_none => {
            //    self.data[self.pos.y as usize].insert(self.pos.x as usize, c);
            //    self.pos.x += 1;
            //    return;
            //}
            (HexMode::Normal, event::Event::Key(mods, c)) if mods == targ_none && c == 'i' => {
                self.mode = HexMode::Insert;
            }
            (_, event::Event::Mouse(pos, _btn)) => {
                self.pos.x = ((pos.x - coords.x) / self.char_size.x / 2) - 5;
                self.pos.x -= self.pos.x / 9;
                self.pos.y = (pos.y - coords.y) / self.char_size.y + self.scroll;
            }
            _ => {}
        }
    }

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("Hex[{}]", self.filename)
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        false
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        lsp.lock()
            .unwrap()
            .close_file(self.filename.clone())
            .unwrap();
        CloseKind::This
    }
}
