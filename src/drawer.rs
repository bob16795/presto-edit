use crate::event::Event;
use crate::highlight::Color;
use crate::math::{Rect, Vector};
use crate::status::Status;
use std::collections::HashMap;

#[derive(PartialEq)]
pub enum CursorStyle {
    Block,
    Bar,
}

pub enum CursorData {
    Hidden,
    Show {
        pos: Vector,
        size: Vector,
        kind: CursorStyle,
    },
}

impl CursorData {
    pub fn offset(&mut self, off: Vector) {
        match self {
            CursorData::Show { pos, .. } => {
                pos.x += off.x;
                pos.y += off.y;
            }
            _ => {}
        }
    }
}

pub enum TextMode {
    Lines,
    Center,
}

pub struct Line {
    pub chars: String,
    pub colors: Vec<Color>,
}

pub trait Handle {
    fn render_text(&self, lines: Vec<Line>, bounds: Rect, mode: TextMode) -> std::io::Result<()>;
    fn render_line(&self, start: Vector, end: Vector) -> std::io::Result<()>;
    fn render_cursor(&self, cur: CursorData) -> std::io::Result<()>;
    fn render_status(&self, st: Status, size: Rect) -> std::io::Result<()>;
    fn get_char_size(&self) -> std::io::Result<Vector>;

    fn end(&self) -> std::io::Result<()>;
}

pub trait Drawer {
    fn init(&mut self) -> std::io::Result<()>;
    fn deinit(&mut self) -> std::io::Result<()>;

    fn begin<'a>(
        &'a mut self,
        colors: &'a HashMap<String, Color>,
    ) -> std::io::Result<Box<dyn Handle + 'a>>;

    fn get_size(&self) -> std::io::Result<Vector>;
    fn get_event(&mut self) -> Option<Event>;
}
