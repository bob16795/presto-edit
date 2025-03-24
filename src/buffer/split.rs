use crate::buffer::*;
use crate::drawer;
use crate::drawer::Drawable;
use crate::event;
use crate::highlight;
use crate::lsp;
use crate::EmptyBuffer;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}
#[derive(Clone)]
pub struct SplitBuffer {
    pub a: Box<Buffer>,
    pub b: Box<Buffer>,
    pub split_dir: SplitDir,
    pub split: Measurement,
    pub a_active: bool,
    pub char_size: Vector,
}

impl BufferFuncs for SplitBuffer {
    fn update(&mut self, size: Vector) {
        match self.split_dir {
            SplitDir::Vertical => {
                let split: i32 = self
                    .split
                    .get_value(size.y as usize, self.char_size.y as usize)
                    as i32;
                let mut sub_size = Vector {
                    x: size.x,
                    y: split,
                };

                self.a.update(sub_size);
                sub_size.y = size.y - split - 1;
                self.b.update(sub_size);
            }
            SplitDir::Horizontal => {
                let split: i32 = self
                    .split
                    .get_value(size.x as usize, self.char_size.x as usize)
                    as i32;
                let mut sub_size = Vector {
                    x: split,
                    y: size.y,
                };

                self.a.update(sub_size);
                sub_size.x = size.x - split - 1;
                self.b.update(sub_size);
            }
        }
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let char_size = handle.get_char_size()?;

        match self.split_dir {
            SplitDir::Vertical => {
                let split: i32 = self
                    .split
                    .get_value(coords.h as usize, char_size.y as usize)
                    as i32;
                self.a.draw(
                    handle,
                    Rect {
                        x: coords.x,
                        y: coords.y,
                        w: coords.w,
                        h: split,
                    },
                )?;
                self.b.draw(
                    handle,
                    Rect {
                        x: coords.x,
                        y: coords.y + split + 1,
                        w: coords.w,
                        h: coords.h - split - 1,
                    },
                )?;
                handle.render_line(
                    Vector {
                        x: coords.x,
                        y: coords.y + split,
                    },
                    Vector {
                        x: coords.x + coords.w,
                        y: coords.y + split,
                    },
                    highlight::Color::Link("split".to_string()),
                )?;
            }
            SplitDir::Horizontal => {
                let split: i32 = self
                    .split
                    .get_value(coords.w as usize, char_size.x as usize)
                    as i32;
                self.a.draw(
                    handle,
                    Rect {
                        x: coords.x,
                        y: coords.y,
                        w: split,
                        h: coords.h,
                    },
                )?;
                self.b.draw(
                    handle,
                    Rect {
                        x: coords.x + split + 1,
                        y: coords.y,
                        w: coords.w - split - 1,
                        h: coords.h,
                    },
                )?;
                handle.render_line(
                    Vector {
                        x: coords.x + split,
                        y: coords.y,
                    },
                    Vector {
                        x: coords.x + split,
                        y: coords.y + coords.h,
                    },
                    highlight::Color::Link("split".to_string()),
                )?;
            }
        }

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData {
        self.char_size = char_size;

        match self.split_dir {
            SplitDir::Vertical => {
                let split: i32 = self.split.get_value(size.y as usize, char_size.y as usize) as i32;
                let sub_size = Vector {
                    x: size.x,
                    y: split,
                };

                let mut result = if self.a_active {
                    self.a.get_cursor(sub_size, char_size)
                } else {
                    self.b.get_cursor(sub_size, char_size)
                };
                if !self.a_active {
                    result.offset(Vector {
                        x: 0,
                        y: (split + 1),
                    });
                }

                result
            }
            SplitDir::Horizontal => {
                let split: i32 = self.split.get_value(size.x as usize, char_size.x as usize) as i32;
                let sub_size = Vector {
                    x: split,
                    y: size.y,
                };

                let mut result = if self.a_active {
                    self.a.get_cursor(sub_size, char_size)
                } else {
                    self.b.get_cursor(sub_size, char_size)
                };

                if !self.a_active {
                    result.offset(Vector {
                        x: (split + 1),
                        y: 0,
                    });
                }

                result
            }
        }
    }

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
        let targ = event::Mods {
            ctrl: true,
            alt: false,
            shift: false,
        };
        match ev {
            event::Event::Nav(mods, event::Nav::Up) if mods == targ => _ = self.nav(NavDir::Up),
            event::Event::Nav(mods, event::Nav::Down) if mods == targ => _ = self.nav(NavDir::Down),
            event::Event::Nav(mods, event::Nav::Left) if mods == targ => _ = self.nav(NavDir::Left),
            event::Event::Nav(mods, event::Nav::Right) if mods == targ => {
                _ = self.nav(NavDir::Right)
            }

            event::Event::Mouse(pos, _btn) => match self.split_dir {
                SplitDir::Horizontal => {
                    let mut new_coords = coords;
                    new_coords.w /= 2;
                    self.a_active = pos.x < new_coords.x + new_coords.w;
                    if self.a_active {
                        self.a.event_process(ev, lsp, new_coords);
                    } else {
                        new_coords.x += new_coords.w;
                        self.b.event_process(ev, lsp, new_coords);
                    }
                }
                SplitDir::Vertical => {
                    let mut new_coords = coords;
                    new_coords.h /= 2;
                    self.a_active = pos.y < new_coords.y + new_coords.h;
                    if self.a_active {
                        self.a.event_process(ev, lsp, new_coords);
                    } else {
                        new_coords.y += new_coords.h;
                        self.b.event_process(ev, lsp, new_coords);
                    }
                }
            },

            _ => match self.split_dir {
                SplitDir::Horizontal => {
                    let mut new_coords = coords;
                    new_coords.w /= 2;
                    if self.a_active {
                        self.a.event_process(ev, lsp, new_coords);
                    } else {
                        new_coords.x += new_coords.w;
                        self.b.event_process(ev, lsp, new_coords);
                    }
                }
                SplitDir::Vertical => {
                    let mut new_coords = coords;
                    new_coords.h /= 2;
                    if self.a_active {
                        self.a.event_process(ev, lsp, new_coords);
                    } else {
                        new_coords.y += new_coords.h;
                        self.b.event_process(ev, lsp, new_coords);
                    }
                }
            },
        }
    }

    fn nav(&mut self, dir: NavDir) -> bool {
        match (dir, self.split_dir) {
            (NavDir::Down, SplitDir::Vertical) => {
                if self.a_active {
                    let success = self.a.nav(dir);

                    if !success {
                        self.a_active = false;
                    }

                    true
                } else {
                    self.b.nav(dir)
                }
            }
            (NavDir::Up, SplitDir::Vertical) => {
                if !self.a_active {
                    let success = self.b.nav(dir);

                    if !success {
                        self.a_active = true;
                    }

                    true
                } else {
                    self.a.nav(dir)
                }
            }
            (NavDir::Left, SplitDir::Horizontal) => {
                if !self.a_active {
                    let success = self.b.nav(dir);

                    if !success {
                        self.a_active = true;
                    }

                    true
                } else {
                    self.a.nav(dir)
                }
            }
            (NavDir::Right, SplitDir::Horizontal) => {
                if self.a_active {
                    let success = self.a.nav(dir);

                    if !success {
                        self.a_active = false;
                    }

                    true
                } else {
                    self.b.nav(dir)
                }
            }
            _ => {
                if self.a_active {
                    self.a.nav(dir)
                } else {
                    self.b.nav(dir)
                }
            }
        }
    }

    fn get_path(&self) -> String {
        if self.a_active {
            "Split>".to_string() + &self.a.get_path()
        } else {
            "Split>".to_string() + &self.b.get_path()
        }
    }

    fn set_focused(&mut self, child: &Box<Buffer>) -> bool {
        if self.a_active {
            if self.a.set_focused(child) {
                self.a = child.clone();
            }
        } else {
            if self.b.set_focused(child) {
                self.b = child.clone();
            }
        }

        return false;
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        if self.a.is_empty() && self.b.is_empty() {
            return CloseKind::This;
        }

        if self.a_active {
            match self.a.close(lsp) {
                CloseKind::Done => CloseKind::Done,
                CloseKind::This => {
                    if self.a.is_empty() {
                        CloseKind::Replace(self.b.clone())
                    } else {
                        self.a = Box::new(EmptyBuffer {}).into();
                        CloseKind::Done
                    }
                }
                CloseKind::Replace(r) => {
                    self.a = r;
                    CloseKind::Done
                }
            }
        } else {
            match self.b.close(lsp) {
                CloseKind::Done => CloseKind::Done,
                CloseKind::This => {
                    if self.b.is_empty() {
                        CloseKind::Replace(self.a.clone())
                    } else {
                        self.b = Box::new(EmptyBuffer {}).into();
                        CloseKind::Done
                    }
                }
                CloseKind::Replace(r) => {
                    self.b = r;
                    CloseKind::Done
                }
            }
        }
    }

    fn focused_child(&mut self) -> Option<&mut Buffer> {
        if self.a_active {
            Some(&mut self.a)
        } else {
            Some(&mut self.b)
        }
    }
}
