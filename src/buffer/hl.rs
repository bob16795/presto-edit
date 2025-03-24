use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use std::collections::HashMap;

#[derive(Clone)]
pub struct HighlightBuffer {
    pub colors: HashMap<String, highlight::Color>,
}

impl BufferFuncs for HighlightBuffer {
    fn update(&mut self, _size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();

        for (c, _v) in &self.colors {
            let mut lc = Vec::new();
            for _ in 0..6 {
                lc.push(highlight::Color::Link(c.to_string()));
            }
            lc.push(highlight::Color::Link("fg".to_string()));

            for _ in c.chars() {
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

    fn get_cursor(&mut self, _size: Vector, _char_size: Vector) -> drawer::CursorData {
        drawer::CursorData::Hidden
    }

    fn event_process(&mut self, _ev: event::Event, _lsp: &mut lsp::LSP, _coords: Rect) {}

    fn nav(&mut self, _dir: NavDir) -> bool {
        false
    }

    fn get_path(&self) -> String {
        "Highlight".to_string()
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        true
    }

    fn close(&mut self, _lsp: &mut lsp::LSP) -> CloseKind {
        CloseKind::This
    }
}
