use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use crate::math::*;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::io::Write;

#[derive(Clone)]
pub struct HighlightBuffer {
    pub colors: HashMap<String, highlight::Color>,
}

impl BufferFuncs for HighlightBuffer {
    fn update(&mut self, size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();

        for (c, v) in &self.colors {
            let mut lc = Vec::new();
            for _ in 0..6 {
                lc.push(highlight::Color::Link(c.to_string()));
            }
            lc.push(highlight::Color::Link("fg".to_string()));

            for ch in c.chars() {
                lc.push(highlight::Color::Link("fg".to_string()));
            }

            lines.push(drawer::Line::Text {
                chars: "XXXXXX ".to_string() + c,
                colors: lc,
            });
        }

        handle.render_text(lines, coords, drawer::TextMode::Lines)?;

        Ok(())
    }

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData {
        drawer::CursorData::Hidden
    }

    fn event_process(&mut self, ev: event::Event, lsp: &mut lsp::LSP, coords: Rect) {}

    fn nav(&mut self, dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Highlight".to_string()
    }

    fn set_focused(&mut self, child: &Box<Buffer>) -> bool {
        true
    }

    fn close(&mut self, lsp: &mut lsp::LSP) -> CloseKind {
        CloseKind::This
    }
}
