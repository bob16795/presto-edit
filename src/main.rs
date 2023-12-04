use crate::drawer::{CursorData, CursorStyle};
use clap::Parser;
use core::ffi::CStr;
use std::collections::HashMap;
use std::fs::{read_dir, read_to_string};
use std::io::{stdout, Write};

use glfw;
use glfw::Context;
use ogl33::*;

mod drawer;
mod drawers {
    pub mod cli;
    pub mod gl;
    pub mod gui;
    pub mod helpers;
}
mod bind;
mod event;
mod highlight;
mod lsp;
mod math;
mod script;
mod status;

use crate::math::{Rect, Vector};
use crate::script::{Command, SplitKind};

enum CloseKind {
    Done,
    This,
    Replace(Box<Buffer>),
}

trait Drawable {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()>;
}

#[derive(Clone)]
struct Buffer {
    vars: HashMap<String, String>,
    base: Box<dyn BufferFuncs>,
}

trait BufferFuncs: CloneBuffer {
    fn setup(&mut self, base: &mut Buffer) {}

    fn update(&mut self, size: Vector);
    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()>;
    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData;
    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect);
    fn nav(&mut self, dir: NavDir) -> bool;
    fn get_path(&self) -> String;
    fn set_focused(&mut self, child: &Box<Buffer>) -> bool;
    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind;

    fn focused_child(&mut self) -> Option<&mut Buffer> {
        None
    }
    fn click(&mut self, pos: Vector, size: Vector) {}
    fn is_empty(&mut self) -> bool {
        false
    }
}

impl<T: BufferFuncs + 'static> From<Box<T>> for Box<Buffer> {
    fn from(base: Box<T>) -> Self {
        let base = base;

        let mut result = Box::new(Buffer {
            vars: HashMap::new(),
            base: Box::new(*base),
        });

        result.base.clone().setup(&mut result);

        result
    }
}

trait CloneBuffer {
    fn clone_buffer<'a>(&self) -> Box<dyn BufferFuncs>;
}

impl<T> CloneBuffer for T
where
    T: BufferFuncs + Clone + 'static,
{
    fn clone_buffer(&self) -> Box<dyn BufferFuncs> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn BufferFuncs> {
    fn clone(&self) -> Self {
        self.clone_buffer()
    }
}

impl Buffer {
    fn set_var(&mut self, v: String, value: String) {
        if let Some(c) = self.base.focused_child() {
            c.set_var(v, value);
        } else {
            self.vars.insert(v, value);
        }
    }

    fn get_var(&mut self, v: &String) -> Option<String> {
        if let Some(c) = self.base.focused_child() {
            if let Some(v) = c.get_var(v) {
                Some(v)
            } else {
                self.vars.get(v).cloned()
            }
        } else {
            self.vars.get(v).cloned()
        }
    }

    fn update(&mut self, size: Vector) {
        self.base.update(size)
    }

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        self.base.draw_conts(handle, coords)
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData {
        self.base.get_cursor(size, char_size)
    }

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
        self.base.event_process(ev, lsp, coords)
    }

    fn nav(&mut self, dir: NavDir) -> bool {
        self.base.nav(dir)
    }

    fn get_path(&self) -> String {
        self.base.get_path()
    }

    fn set_focused(&mut self, child: &Box<Buffer>) -> bool {
        self.base.set_focused(child)
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        self.base.close(lsp)
    }

    fn click(&mut self, pos: Vector, size: Vector) {
        self.base.click(pos, size)
    }

    fn is_empty(&mut self) -> bool {
        self.base.is_empty()
    }
}

impl Drawable for Buffer {
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
    tabs: Vec<Box<Buffer>>,
    active: usize,
    char_size: Vector,
}

impl BufferFuncs for TabbedBuffer {
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
        new_coords.y += self.char_size.y;
        new_coords.h -= self.char_size.y;

        self.tabs[self.active].draw(handle, new_coords)?;

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> CursorData {
        self.char_size = char_size;
        let mut result = self.tabs[self.active].get_cursor(size, char_size);
        result.offset(Vector {
            x: 0,
            y: char_size.y,
        });
        result
    }

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
        let mut new_coords = coords;
        new_coords.y += self.char_size.y;
        new_coords.h -= self.char_size.y;

        self.tabs[self.active].event_process(ev, lsp, new_coords);
    }

    fn nav(&mut self, _dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Tabs>".to_string() + &self.tabs[self.active].get_path()
    }

    fn set_focused(&mut self, child: &Box<Buffer>) -> bool {
        if self.tabs[self.active].set_focused(child) {
            self.tabs[self.active] = child.clone();
        }

        return false;
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
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

        match self.tabs[self.active].close(lsp) {
            CloseKind::Done => CloseKind::Done,
            CloseKind::This => {
                self.tabs[self.active] = Box::new(EmptyBuffer {}).into();
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
    a: Box<Buffer>,
    b: Box<Buffer>,
    split_dir: SplitDir,
    split: Measurement,
    a_active: bool,
    char_size: Vector,
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

            event::Event::Mouse(pos, btn) => match self.split_dir {
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

    fn focused_child(&mut self) -> Option<&mut Buffer> {
        if self.a_active {
            Some(&mut self.a)
        } else {
            Some(&mut self.b)
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

impl BufferFuncs for TreeBuffer {
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

    fn event_process(&mut self, _ev: event::Event, _lsp: &mut lsp::LSP, _coords: Rect) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("Tree[{}]", self.path.display())
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        false
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        CloseKind::This
    }
}

#[derive(Clone)]
struct TextBuffer {
    text: String,
}

impl BufferFuncs for TextBuffer {
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

    fn event_process(&mut self, _ev: event::Event, _lsp: &mut lsp::LSP, _coords: Rect) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        return false;
    }

    fn get_path(&self) -> String {
        format!("Text")
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        false
    }

    fn close(&mut self, _lsp: &mut lsp::LSP) -> CloseKind {
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
    char_size: Vector,
}

impl BufferFuncs for FileBuffer {
    fn setup(&mut self, base: &mut Buffer) {
        base.vars.insert(
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

        self.char_size = char_size;

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

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
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
            (_, event::Event::Mouse(pos, btn)) => {
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

#[derive(Clone)]
struct EmptyBuffer {}

fn create_line(text: String) -> drawer::Line {
    let mut colors = Vec::new();
    for _ in 0..text.len() {
        colors.push(highlight::Color::Link("fg".to_string()));
    }

    drawer::Line {
        colors,
        chars: text,
    }
}

impl BufferFuncs for EmptyBuffer {
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

    fn event_process(&mut self, _ev: event::Event, _lsp: &mut lsp::LSP, _coords: Rect) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Empty".to_string()
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        true
    }

    fn close(&mut self, _lsp: &mut lsp::LSP) -> CloseKind {
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
    ft: String,
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
                right: self.ft.clone() + &" | PrestoEdit".to_string(),
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
        bu: &mut Box<Buffer>,
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

        let mut done = false;

        while !done {
            for ev in (drawer as &mut dyn drawer::Drawer).get_events() {
                match ev {
                    event::Event::Nav(mods, event::Nav::Escape) if mods == targ_none => {
                        self.prompt = None;

                        return Ok(None);
                    }
                    event::Event::Nav(mods, event::Nav::Enter) if mods == targ_none => done = true,
                    event::Event::Nav(mods, event::Nav::BackSpace) if mods == targ_none => {
                        _ = self.input.pop()
                    }
                    event::Event::Key(mods, c) if mods == targ_none => self.input.push(c),
                    event::Event::Quit => done = true,
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
    bu: &'a mut Buffer,
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

    let cur = bu.get_cursor(
        Vector {
            x: size.x as i32,
            y: size.y as i32,
        },
        handle.get_char_size()?,
    );
    handle.render_cursor(cur)?;

    status.path = bu.get_path();
    status.ft = format!("{:?}", bu.get_var(&"filetype".to_string()));

    status.draw(
        handle,
        Rect {
            x: 0,
            y: size.y - 1,
            w: size.x as i32,
            h: 1,
        },
    )?;

    handle.end()?;

    Ok(())
}

fn run_command<'a, 'b>(
    cmd: Command,
    dr: &mut dyn drawer::Drawer,
    bu: &mut Box<Buffer>,
    status: &mut Status,
    binds: &mut HashMap<String, Command>,
    colors: &mut HashMap<String, highlight::Color>,
    lsp: &mut lsp::LSP,
) -> std::io::Result<()> {
    match cmd {
        Command::Unknown(_) => {}
        Command::Incomplete(cmd) => {
            if let Some(cmd) =
                status.prompt("".to_string(), dr, bu, cmd.to_string() + " ", colors)?
            {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            };
        }
        Command::Split(SplitKind::Horizontal) => {
            let adds: Box<Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}).into(),
                b: Box::new(EmptyBuffer {}).into(),
                split_dir: SplitDir::Horizontal,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Vertical) => {
            let adds: Box<Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}).into(),
                b: Box::new(EmptyBuffer {}).into(),
                split_dir: SplitDir::Vertical,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Tabbed) => {
            let adds: Box<Buffer> = Box::new(TabbedBuffer {
                tabs: vec![Box::new(EmptyBuffer {}).into()],
                active: 0,
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Open(path) => {
            let cont = read_to_string(&path);
            let adds: Box<Buffer> = Box::new(FileBuffer {
                filename: path.clone(),
                cached: false,
                data: Vec::new(),
                pos: Vector { x: 0, y: 0 },
                scroll: 0,
                mode: FileMode::Normal,
                height: 0,
                char_size: Vector { x: 0, y: 0 },
            })
            .into();
            if let Ok(c) = cont {
                lsp.open_file(path, c)?;
            }
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Write(path) => {
            bu.as_mut().event_process(
                event::Event::Save(path),
                lsp,
                Rect {
                    x: 0,
                    y: 0,
                    w: dr.get_size()?.x,
                    h: dr.get_size()?.y,
                },
            );
        }
        Command::Source(path) => {
            let file = read_to_string(&path)?;
            for line in file.lines() {
                let cmd = Command::parse(line.to_string());

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            }
        }
        Command::Run => {
            if let Some(cmd) = status.prompt("".to_string(), dr, bu, "".to_string(), &colors)? {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            };
        }
        Command::Close => match bu.close(lsp) {
            CloseKind::Replace(r) => *bu = r,
            CloseKind::This => *bu = Box::new(EmptyBuffer {}).into(),
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
        Command::Set(s, None) => {
            println!("{:?}", bu.get_var(&s));
        }
        Command::Set(s, Some(v)) => {
            bu.set_var(s, v);
        }
        c => {
            println!("todo{:?}", c)
        }
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
        let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();
        glfw.window_hint(glfw::WindowHint::Samples(Some(4)));

        let (mut win, events) = glfw
            .create_window(1366, 768, "PrestoEdit", glfw::WindowMode::Windowed)
            .unwrap();

        unsafe {
            load_gl_with(|f_name| win.get_proc_address(CStr::from_ptr(f_name).to_str().unwrap()))
        }
        win.make_current();
        win.set_all_polling(true);

        glfw.set_swap_interval(glfw::SwapInterval::Adaptive);

        let font = drawers::gl::GlFont::new("font.ttf");

        drawer_box = Box::new(drawers::gl::GlDrawer {
            glfw,
            win: std::cell::RefCell::new(win),
            events,
            size: Vector { x: 640, y: 480 },
            font: std::cell::RefCell::new(font),
            keys: HashMap::new(),
            solid_program: std::cell::RefCell::new(None),
            cursor: std::cell::RefCell::new([drawers::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
            cursor_targ: std::cell::RefCell::new([drawers::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
            cursor_t: std::cell::RefCell::new([0.0; 4]),
            mods: event::Mods {
                shift: false,
                alt: false,
                ctrl: false,
            },
            mouse: Vector { x: 0, y: 0 },
        });

        //let (mut rl, thread) = raylib::init()
        //    .msaa_4x()
        //    .resizable()
        //    .title("PrestoEdit")
        //    .build();
        //rl.set_target_fps(60);
        //drawer_box = Box::new(drawers::gui::GuiDrawer {
        //    rl,
        //    thread,
        //    font: None,
        //    cursor: std::cell::RefCell::new([
        //        raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
        //    ]),
        //    cursor_targ: std::cell::RefCell::new([
        //        raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
        //    ]),
        //    cursor_t: std::cell::RefCell::new([0.0; 4]),
        //});
    };

    let drawer: &mut dyn drawer::Drawer = drawer_box.as_mut();
    drawer.init()?;

    let mut binds = HashMap::new();
    let mut colors = HashMap::new();
    let mut bu: Box<Buffer> = Box::new(EmptyBuffer {}).into();
    let mut status = Status {
        path: "".to_string(),
        prompt: None,
        input: "".to_string(),
        ft: "".to_string(),
    };

    let mut lsp = lsp::LSP::new();
    lsp.init()?;

    let cmd = Command::parse("source /home/john/.config/prestoedit/init.pe".to_string());
    run_command(
        cmd,
        drawer,
        &mut bu,
        &mut status,
        &mut binds,
        &mut colors,
        &mut lsp,
    )?;

    binds.insert("<S-:>".to_string(), Command::Run);

    render(drawer, bu.as_mut(), &mut status, &colors)?;

    let mut done = false;

    while !done {
        for ev in drawer.get_events() {
            match &ev {
                event::Event::Quit => done = true,
                _ => {
                    if let Some(cmd) = bind::check(&mut binds, &ev) {
                        run_command(
                            cmd,
                            drawer,
                            &mut bu,
                            &mut status,
                            &mut binds,
                            &mut colors,
                            &mut lsp,
                        )?;
                    } else {
                        bu.as_mut().event_process(
                            ev,
                            &mut lsp,
                            Rect {
                                x: 0,
                                y: 0,
                                w: drawer.get_size()?.x,
                                h: drawer.get_size()?.y,
                            },
                        )
                    };
                }
            }
        }
        render(drawer, bu.as_mut(), &mut status, &colors)?;
    }

    drawer.deinit()?;

    Ok(())
}
