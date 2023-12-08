use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::lsp;
use crate::math::*;
use std::fs::read_dir;

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

            for _ in 0..file.1.len() {
                colors.push(highlight::Color::Link("fg".to_string()));
            }

            lines.push(drawer::Line::Text { chars, colors });
        }

        handle.render_text(lines, coords, drawer::TextMode::Lines)?;

        Ok(())
    }

    fn get_cursor(&mut self, _size: Vector, char_size: Vector) -> drawer::CursorData {
        drawer::CursorData::Show {
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

    fn close(&mut self, _lsp: &mut lsp::LSP) -> CloseKind {
        CloseKind::This
    }
}
