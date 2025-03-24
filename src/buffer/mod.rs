use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use crate::math::*;
use std::collections::HashMap;

pub mod empty;
pub mod file;
pub mod hex;
pub mod hl;
pub mod logview;
pub mod split;
pub mod tabbed;
//pub mod tree;

#[derive(Debug, Copy, Clone)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

pub enum CloseKind {
    Done,
    This,
    Replace(Box<Buffer>),
}

#[derive(Clone)]
pub struct Buffer {
    pub vars: HashMap<String, String>,
    pub base: Box<dyn BufferFuncs>,
}

pub trait BufferFuncs: CloneBuffer {
    fn setup(&mut self, _base: &mut Buffer) {}

    fn update(&mut self, size: Vector);
    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()>;
    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData;
    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect);
    fn nav(&mut self, dir: NavDir) -> bool;
    fn get_path(&self) -> String;
    fn set_focused(&mut self, child: &Box<Buffer>) -> bool;
    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind;

    fn setup_lsp(&mut self, _lsp: &mut lsp::LSP) {}

    fn focused_child(&mut self) -> Option<&mut Buffer> {
        None
    }
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

pub trait CloneBuffer {
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
    pub fn set_var(&mut self, v: String, value: String) {
        if let Some(c) = self.base.focused_child() {
            c.set_var(v, value);
        } else {
            self.vars.insert(v, value);
        }
    }

    pub fn get_var(&mut self, v: &String) -> Option<String> {
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

    pub fn update(&mut self, size: Vector) {
        self.base.update(size)
    }

    pub fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        self.base.draw_conts(handle, coords)
    }

    pub fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData {
        self.base.get_cursor(size, char_size)
    }

    pub fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {
        self.base.event_process(ev, lsp, coords)
    }

    pub fn nav(&mut self, dir: NavDir) -> bool {
        self.base.nav(dir)
    }

    pub fn get_path(&self) -> String {
        self.base.get_path()
    }

    pub fn set_focused(&mut self, child: &Box<Buffer>) -> bool {
        self.base.set_focused(child)
    }

    pub fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        self.base.close(lsp)
    }

    pub fn is_empty(&mut self) -> bool {
        self.base.is_empty()
    }

    pub fn setup_lsp(&mut self, lsp: &mut lsp::LSP) {
        self.base.setup_lsp(lsp);
    }
}

impl drawer::Drawable for Buffer {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        self.draw_conts(handle, coords)?;

        Ok(())
    }
}

pub fn create_line(text: String) -> drawer::Line {
    let mut colors = Vec::new();
    for _ in 0..text.len() {
        colors.push(highlight::Color::Link("fg".to_string()));
    }

    drawer::Line::Text {
        colors,
        chars: text,
    }
}
