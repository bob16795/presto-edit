use crate::drawer::{CursorData, CursorStyle};
use clap::Parser;
use std::collections::HashMap;
use std::fs::{read_dir, read_to_string};
use std::io::{stdout, Write};

mod drawer;
mod drawers {
    pub mod cli;
    pub mod gui;
}
mod bind;
mod event;
mod highlight;
mod math;
mod script;
mod status;

use crate::math::{Rect, Vector};
use crate::script::{Command, SplitKind};

enum CloseKind {
    Done,
    This,
    Replace(Box<dyn Buffer>),
}

trait Drawable {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()>;
}

//struct Buffer {
//    vars: HashMap<String, String>,
//    base: Box<dyn BufferData>,
//}

trait Buffer: CloneBuffer {
    fn update(&mut self, size: Vector);
    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()>;
    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData;
    fn event_process(&mut self, ev: event::Event);
    fn nav(&mut self, dir: NavDir) -> bool;
    fn get_path(&self) -> String;
    fn set_focused(&mut self, child: &Box<dyn Buffer>) -> bool;
    fn close(&mut self) -> CloseKind;

    fn click(&mut self, pos: Vector, size: Vector) {}
    fn is_empty(&mut self) -> bool {
        false
    }
}

trait CloneBuffer {
    fn clone_buffer<'a>(&self) -> Box<dyn Buffer>;
}

impl<T> CloneBuffer for T
where
    T: Buffer + Clone + 'static,
{
    fn clone_buffer(&self) -> Box<dyn Buffer> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Buffer> {
    fn clone(&self) -> Self {
        self.clone_buffer()
    }
}

impl Drawable for dyn Buffer + '_ {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        self.draw_conts(handle, coords)?;

        Ok(())
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
enum SplitDir {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
enum Measurement {
    Percent(f32),
    Chars(usize),
    NegChars(usize),
    Pixels(usize),
    NegPixels(usize),
}

impl Measurement {
    fn get_value(&self, max: usize, char_size: usize) -> usize {
        match &self {
            Self::Percent(pc) => (max as f32 * pc) as usize,
            Self::Chars(val) => (*val * char_size).min(max),
            Self::NegChars(val) => max - (*val * char_size).min(max),
            Self::Pixels(val) => (*val).min(max),
            Self::NegPixels(val) => max - val.min(&max),
        }
    }
}

#[derive(Clone)]
struct TabbedBuffer {
    tabs: Vec<Box<dyn Buffer>>,
    active: usize,
}

impl Buffer for TabbedBuffer {
    fn update(&mut self, size: Vector) {
        let sub_size = Vector {
            x: size.x,
            y: size.y - 1,
        };
        for tab in &mut self.tabs {
            tab.update(sub_size)
        }
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut new_coords = coords;
        new_coords.y += handle.get_char_size()?.y;
        new_coords.h -= handle.get_char_size()?.y;

        self.tabs[self.active].draw(handle, new_coords)?;

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData {
        let mut result = self.tabs[self.active].get_cursor(size, char_size);
        result.offset(Vector {
            x: 0,
            y: char_size.y,
        });
        result
    }

    fn event_process(&mut self, ev: event::Event) {
        self.tabs[self.active].event_process(ev);
    }

    fn nav(&mut self, _dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Tabs>".to_string() + &self.tabs[self.active].get_path()
    }

    fn set_focused(&mut self, child: &Box<dyn Buffer>) -> bool {
        if self.tabs[self.active].set_focused(child) {
            self.tabs[self.active] = child.clone();
        }

        return false;
    }

    fn close(&mut self) -> CloseKind {
        if self.tabs[self.active].is_empty() {
            self.tabs.remove(self.active);
            if self.active != 0 {
                self.active -= 1;
            }

            if self.tabs.len() == 0 {
                return CloseKind::This;
            }

            return CloseKind::Done;
        }

        match self.tabs[self.active].close() {
            CloseKind::Done => CloseKind::Done,
            CloseKind::This => {
                self.tabs[self.active] = Box::new(EmptyBuffer {});
                CloseKind::Done
            }
            CloseKind::Replace(r) => {
                self.tabs[self.active] = r;
                CloseKind::Done
            }
        }
    }
}

#[derive(Clone)]
struct SplitBuffer {
    a: Box<dyn Buffer>,
    b: Box<dyn Buffer>,
    split_dir: SplitDir,
    split: Measurement,
    a_active: bool,
    char_size: Vector,
}

impl Buffer for SplitBuffer {
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
                handle.render_line(
                    Vector {
                        x: coords.x,
                        y: coords.y + split,
                    },
                    Vector {
                        x: coords.x + coords.w,
                        y: coords.y + split,
                    },
                )?;

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
            }
            SplitDir::Horizontal => {
                let split: i32 = self
                    .split
                    .get_value(coords.w as usize, char_size.x as usize)
                    as i32;
                handle.render_line(
                    Vector {
                        x: coords.x + split,
                        y: coords.y,
                    },
                    Vector {
                        x: coords.x + split,
                        y: coords.y + coords.h,
                    },
                )?;
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
            }
        }

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData {
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

    fn event_process(&mut self, ev: event::Event) {
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

            _ => {
                if self.a_active {
                    self.a.event_process(ev)
                } else {
                    self.b.event_process(ev)
                }
            }
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

    fn set_focused(&mut self, child: &Box<dyn Buffer>) -> bool {
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

    fn close(&mut self) -> CloseKind {
        if self.a.is_empty() && self.b.is_empty() {
            return CloseKind::This;
        }

        if self.a_active {
            match self.a.close() {
                CloseKind::Done => CloseKind::Done,
                CloseKind::This => {
                    if self.a.is_empty() {
                        CloseKind::Replace(self.b.clone())
                    } else {
                        self.a = Box::new(EmptyBuffer {});
                        CloseKind::Done
                    }
                }
                CloseKind::Replace(r) => {
                    self.a = r;
                    CloseKind::Done
                }
            }
        } else {
            match self.b.close() {
                CloseKind::Done => CloseKind::Done,
                CloseKind::This => {
                    if self.b.is_empty() {
                        CloseKind::Replace(self.a.clone())
                    } else {
                        self.b = Box::new(EmptyBuffer {});
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

    fn click(&mut self, pos: Vector, size: Vector) {
        match self.split_dir {
            SplitDir::Horizontal => {
                self.a_active = (size.x / 2) < pos.x;
                //if self.a_active {
                //    self.a.click();
                //}
            }
            SplitDir::Vertical => {
                self.a_active = (size.y / 2) < pos.y;
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone)]
struct TreeBuffer {
    path: std::path::PathBuf,
    cache: Vec<(char, String)>,
    cached: bool,
}

impl Buffer for TreeBuffer {
    fn update(&mut self, _size: Vector) {
        if !self.cached {
            for file in read_dir(&self.path).unwrap() {
                let label = if file.as_ref().unwrap().file_type().unwrap().is_dir() {
                    'D'
                } else {
                    'F'
                };

                let path = &file.unwrap().path();
                let path = path
                    .strip_prefix(&self.path)
                    .unwrap()
                    .as_os_str()
                    .to_string_lossy();

                self.cache.push((label, path.to_string()));
            }

            self.cached = true;
        }

        self.cache.sort_by(|a, b| {
            (a.0.to_string() + a.1.as_str())
                .partial_cmp(&(b.0.to_string() + b.1.as_str()))
                .unwrap()
        })
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();

        for file in &self.cache {
            let chars = format!("{} {}", file.0, file.1);
            let mut colors = Vec::new();

            colors.push(highlight::Color::Link("label".to_string()));
            colors.push(highlight::Color::Link("label".to_string()));

            for c in 0..file.1.len() {
                colors.push(highlight::Color::Link("fg".to_string()));
            }

            lines.push(drawer::Line { chars, colors });
        }

        handle.render_text(lines, coords, drawer::TextMode::Lines)?;

        Ok(())
    }

    fn get_cursor(&mut self, _size: Vector, char_size: Vector) -> CursorData {
        CursorData::Show {
            pos: Vector { x: 0, y: 0 },
            size: char_size,
            kind: drawer::CursorStyle::Block,
        }
    }

    fn event_process(&mut self, _ev: event::Event) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("Tree[{}]", self.path.display())
    }

    fn set_focused(&mut self, _child: &Box<dyn Buffer>) -> bool {
        false
    }

    fn close(&mut self) -> CloseKind {
        CloseKind::This
    }
}

#[derive(Clone)]
struct TextBuffer {
    text: String,
}

impl Buffer for TextBuffer {
    fn update(&mut self, _size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        handle.render_text(
            vec![create_line(self.text.clone())],
            coords,
            drawer::TextMode::Lines,
        )?;

        Ok(())
    }

    fn get_cursor(&mut self, _size: Vector, char_size: Vector) -> CursorData {
        CursorData::Show {
            pos: Vector {
                x: self.text.len() as i32 * char_size.x,
                y: 0,
            },
            size: char_size,
            kind: drawer::CursorStyle::Block,
        }
    }

    fn event_process(&mut self, _ev: event::Event) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("Text")
    }

    fn set_focused(&mut self, _child: &Box<dyn Buffer>) -> bool {
        false
    }

    fn close(&mut self) -> CloseKind {
        CloseKind::This
    }
}

#[derive(PartialEq, Clone)]
enum FileMode {
    Normal,
    Insert,
}

#[derive(Clone)]
struct FileBuffer {
    filename: String,
    cached: bool,
    data: Vec<String>,
    pos: Vector,
    scroll: i32,
    mode: FileMode,
    height: i32,
}

impl Buffer for FileBuffer {
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

        while self.pos.y - self.scroll < 5 && self.scroll > 0 {
            self.scroll -= 1;
        }
        while self.pos.y - self.scroll > self.height - 5 && self.scroll < self.data.len() as i32 {
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
                lines.push(drawer::Line {
                    chars: format!(" "),
                    colors: vec![highlight::Color::Link("number".to_string())],
                });
                continue;
            }

            let l = &self.data[line_idx as usize];
            let line = format!("{:>4} {}", line_idx + 1, l);
            let mut colors = Vec::new();

            for c in 0..5 {
                colors.push(highlight::Color::Link("number".to_string()));
            }

            for ch in l.chars() {
                if ch.is_numeric() {
                    colors.push(highlight::Color::Link("fg".to_string()));
                } else {
                    colors.push(highlight::Color::Link("fg".to_string()));
                }
            }

            lines.push(drawer::Line {
                chars: line,
                colors,
            });
        }

        let w = handle.get_char_size()?.x;

        handle.render_text(lines, coords, drawer::TextMode::Lines)?;

        handle.render_line(
            Vector {
                x: coords.x + (w as f32 * 4.5) as i32,
                y: coords.y,
            },
            Vector {
                x: coords.x + (w as f32 * 4.5) as i32,
                y: coords.y + coords.h,
            },
        )?;

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData {
        self.height = size.y / char_size.y;

        let mut result = CursorData::Show {
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

    fn event_process(&mut self, ev: event::Event) {
        let targ_none = event::Mods {
            ctrl: false,
            alt: false,
            shift: false,
        };
        let targ_ctrl = event::Mods {
            ctrl: true,
            alt: false,
            shift: false,
        };

        match self.mode {
            FileMode::Insert => match ev {
                event::Event::Nav(mods, event::Nav::Down) if mods == targ_none => {
                    self.pos.y += 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Up) if mods == targ_none => {
                    self.pos.y -= 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Left) if mods == targ_none => {
                    self.pos.x -= 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Right) if mods == targ_none => {
                    self.pos.x += 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Enter) if mods == targ_none => {
                    let next = self.data[self.pos.y as usize].split_off(self.pos.x as usize);
                    self.data.insert((self.pos.y + 1) as usize, next);
                    self.pos.x = 0;
                    self.pos.y += 1;

                    return;
                }
                event::Event::Nav(mods, event::Nav::BackSpace) if mods == targ_none => {
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
                event::Event::Nav(mods, event::Nav::Escape) if mods == targ_none => {
                    self.mode = FileMode::Normal;
                }
                event::Event::Save(None) => {
                    let mut file = std::fs::File::create(self.filename.as_str()).unwrap();
                    for line in &self.data {
                        let _ = file.write(line.as_bytes());
                        let _ = file.write(b"\n");
                    }
                }
                event::Event::Key(mods, c) if mods == targ_none => {
                    self.data[self.pos.y as usize].insert(self.pos.x as usize, c);
                    self.pos.x += 1;
                    return;
                }
                _ => {}
            },
            FileMode::Normal => match ev {
                event::Event::Nav(mods, event::Nav::Down) if mods == targ_none => {
                    self.pos.y += 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Up) if mods == targ_none => {
                    self.pos.y -= 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Left) if mods == targ_none => {
                    self.pos.x -= 1;
                    return;
                }
                event::Event::Nav(mods, event::Nav::Right) if mods == targ_none => {
                    self.pos.x += 1;
                    return;
                }
                event::Event::Key(mods, c) if mods == targ_none && c == 'i' => {
                    self.mode = FileMode::Insert;
                }
                event::Event::Save(None) => {
                    let mut file = std::fs::File::create(self.filename.as_str()).unwrap();
                    for line in &self.data {
                        let _ = file.write(line.as_bytes());
                        let _ = file.write(b"\n");
                    }
                }
                _ => {}
            },
        }
    }

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("File[{}]", self.filename)
    }

    fn set_focused(&mut self, _child: &Box<dyn Buffer>) -> bool {
        false
    }

    fn close(&mut self) -> CloseKind {
        CloseKind::This
    }
}

#[derive(Clone)]
struct EmptyBuffer {}

fn create_line(text: String) -> drawer::Line {
    let mut colors = Vec::new();
    for t in 0..text.len() {
        colors.push(highlight::Color::Link("fg".to_string()));
    }

    drawer::Line {
        colors,
        chars: text,
    }
}

impl Buffer for EmptyBuffer {
    fn update(&mut self, _size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        handle.render_text(
            vec![
                create_line("        EMPTY BUFFER        ".to_string()),
                create_line("Press Ctrl-O to open a file!".to_string()),
            ],
            coords,
            drawer::TextMode::Center,
        )?;

        Ok(())
    }

    fn get_cursor(&mut self, _size: Vector, char_size: Vector) -> CursorData {
        CursorData::Show {
            pos: Vector { x: 0, y: 0 },
            size: char_size,
            kind: CursorStyle::Bar,
        }
    }

    fn event_process(&mut self, _ev: event::Event) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Empty".to_string()
    }

    fn set_focused(&mut self, _child: &Box<dyn Buffer>) -> bool {
        true
    }

    fn close(&mut self) -> CloseKind {
        CloseKind::This
    }

    fn is_empty(&mut self) -> bool {
        true
    }
}

struct Status {
    path: String,
    prompt: Option<String>,
    input: String,
}

impl Drawable for Status {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let left = match &self.prompt {
            Some(p) => format!("{}:{}", p, self.input),
            None => format!("{}", self.path),
        };

        handle.render_status(
            status::Status {
                left,
                center: "".to_string(),
                right: "PrestoEdit".to_string(),
            },
            coords,
        )?;

        Ok(())
    }
}

impl Status {
    fn prompt<'a>(
        &mut self,
        input: String,
        drawer: &mut dyn drawer::Drawer,
        bu: &mut Box<dyn Buffer>,
        default: String,
        colors: &'a HashMap<String, highlight::Color>,
    ) -> std::io::Result<Option<String>> {
        self.prompt = Some(input);
        self.input = default;

        render(drawer, bu.as_mut(), self, colors)?;

        let targ_none = event::Mods {
            ctrl: false,
            alt: false,
            shift: false,
        };

        loop {
            if let Some(ev) = (drawer as &mut dyn drawer::Drawer).get_event() {
                match ev {
                    event::Event::Nav(mods, event::Nav::Escape) if mods == targ_none => {
                        self.prompt = None;

                        return Ok(None);
                    }
                    event::Event::Nav(mods, event::Nav::Enter) if mods == targ_none => break,
                    event::Event::Nav(mods, event::Nav::BackSpace) if mods == targ_none => {
                        _ = self.input.pop()
                    }
                    event::Event::Key(mods, c) if mods == targ_none => self.input.push(c),
                    event::Event::Quit => break,
                    _ => {}
                }
            }
            render(drawer, bu.as_mut(), self, colors)?;
        }

        self.prompt = None;

        render(drawer, bu.as_mut(), self, colors)?;

        Ok(Some(self.input.clone()))
    }
}

fn render<'a, 'c, 'd, 'e>(
    drawer: &mut dyn drawer::Drawer,
    bu: &'a mut dyn Buffer,
    status: &'c mut Status,
    colors: &'e HashMap<String, highlight::Color>,
) -> std::io::Result<()> {
    let size = drawer.get_size()?;
    bu.update(size);

    let mut handle = drawer.begin(colors)?;
    let handle = handle.as_mut();

    bu.draw(
        handle,
        Rect {
            x: 0,
            y: 0,
            w: size.x as i32,
            h: size.y as i32,
        },
    )?;
    status.path = bu.get_path();

    status.draw(
        handle,
        Rect {
            x: 0,
            y: size.y - 1,
            w: size.x as i32,
            h: 1,
        },
    )?;

    let cur = bu.get_cursor(
        Vector {
            x: size.x as i32,
            y: size.y as i32,
        },
        handle.get_char_size()?,
    );

    handle.render_cursor(cur)?;

    handle.end()?;

    Ok(())
}

fn run_command<'a, 'b>(
    cmd: Command,
    dr: &mut dyn drawer::Drawer,
    bu: &mut Box<dyn Buffer>,
    status: &mut Status,
    binds: &mut HashMap<String, Command>,
    colors: &mut HashMap<String, highlight::Color>,
) -> std::io::Result<()> {
    match cmd {
        Command::Unknown(_) => {}
        Command::Incomplete(cmd) => {
            if let Some(cmd) =
                status.prompt("".to_string(), dr, bu, cmd.to_string() + " ", colors)?
            {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors)?;
            };
        }
        Command::Split(SplitKind::Horizontal) => {
            let adds: Box<dyn Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}),
                b: Box::new(EmptyBuffer {}),
                split_dir: SplitDir::Horizontal,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            });
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Vertical) => {
            let adds: Box<dyn Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}),
                b: Box::new(EmptyBuffer {}),
                split_dir: SplitDir::Vertical,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            });
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Tabbed) => {
            let adds: Box<dyn Buffer> = Box::new(TabbedBuffer {
                tabs: vec![Box::new(EmptyBuffer {})],
                active: 0,
            });
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Open(path) => {
            let adds: Box<dyn Buffer> = Box::new(FileBuffer {
                filename: path,
                cached: false,
                data: Vec::new(),
                pos: Vector { x: 0, y: 0 },
                scroll: 0,
                mode: FileMode::Normal,
                height: 0,
            });
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Write(path) => {
            bu.as_mut().event_process(event::Event::Save(path));
        }
        Command::Source(path) => {
            let file = read_to_string(&path)?;
            for line in file.lines() {
                let cmd = Command::parse(line.to_string());

                run_command(cmd, dr, bu, status, binds, colors)?;
            }
        }
        Command::Run => {
            if let Some(cmd) = status.prompt("".to_string(), dr, bu, "".to_string(), &colors)? {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors)?;
            };
        }
        Command::Close => match bu.close() {
            CloseKind::Replace(r) => *bu = r,
            CloseKind::This => *bu = Box::new(EmptyBuffer {}),
            CloseKind::Done => {}
        },
        Command::Highlight(s, None) => {
            colors.remove(&s);
        }
        Command::Highlight(s, Some(c)) => {
            colors.insert(s, c);
        }
        Command::Bind(s, None) => {
            binds.remove(&s);
        }
        Command::Bind(s, Some(c)) => {
            binds.insert(s, *c);
        }
        _ => todo!("{:?}", cmd),
    }
    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "false")]
    cmd: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    let mut drawer_box: Box<dyn drawer::Drawer>;

    if args.cmd {
        drawer_box = Box::new(drawers::cli::CliDrawer { stdout: stdout() });
    } else {
        let (mut rl, thread) = raylib::init()
            .msaa_4x()
            .resizable()
            .title("Hello, World")
            .build();
        rl.set_target_fps(60);
        drawer_box = Box::new(drawers::gui::GuiDrawer {
            rl,
            thread,
            font: None,
            cursor: std::cell::RefCell::new([
                raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
                raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
                raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
                raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
            ]),
            cursor_targ: std::cell::RefCell::new([
                raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
                raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
                raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
                raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
            ]),
            cursor_t: std::cell::RefCell::new([0.0; 4]),
        });
    };

    let drawer: &mut dyn drawer::Drawer = drawer_box.as_mut();

    let mut binds = HashMap::new();
    let mut colors = HashMap::new();
    let mut bu: Box<dyn Buffer> = Box::new(EmptyBuffer {}) as Box<dyn Buffer>;
    let mut status = Status {
        path: "".to_string(),
        prompt: None,
        input: "".to_string(),
    };

    drawer.init()?;

    let cmd = Command::parse("source /home/john/.config/prestoedit/init.pe".to_string());
    run_command(cmd, drawer, &mut bu, &mut status, &mut binds, &mut colors)?;

    binds.insert("<:>".to_string(), Command::Run);

    render(drawer, bu.as_mut(), &mut status, &colors)?;

    loop {
        if let Some(ev) = drawer.get_event() {
            match &ev {
                event::Event::Quit => break,
                _ => {
                    if let Some(cmd) = bind::check(&mut binds, &ev) {
                        run_command(cmd, drawer, &mut bu, &mut status, &mut binds, &mut colors)?;
                    } else {
                        bu.as_mut().event_process(ev)
                    };
                }
            }
        }
        render(drawer, bu.as_mut(), &mut status, &colors)?;
    }

    drawer.deinit()?;

    Ok(())
}
