use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::highlight;
use crate::logging;
use crate::lsp;

#[derive(Clone)]
pub struct LogViewBuffer {}

impl BufferFuncs for LogViewBuffer {
    fn update(&mut self, _size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let mut lines = Vec::new();

        for line in (&*logging::get_lines()).into_iter().rev() {
            let mut lc = Vec::new();

            let text = format!("{: >15} {}", line.target, line.text);
            let color = match line.level {
                log::Level::Error => highlight::Color::Link("log_error".to_string()),
                log::Level::Warn => highlight::Color::Link("log_warn".to_string()),
                log::Level::Info => highlight::Color::Link("log_info".to_string()),
                _ => highlight::Color::Link("fg".to_string()),
            };

            for i in 0..text.to_string().len() {
                if i <= 15 {
                    lc.push(highlight::Color::Link("lineNumberFg".to_string()));
                } else {
                    lc.push(color.clone());
                }
            }

            lines.push(drawer::Line::Text {
                chars: text,
                colors: lc,
            });
        }

        let w = handle.get_char_size()?.x;

        handle.render_rect(
            Vector {
                x: coords.x,
                y: coords.y,
            },
            Vector {
                x: (w as f32 * 15.5) as i32,
                y: coords.h,
            },
            highlight::Color::Link("lineNumberBg".to_string()),
        )?;

        handle.render_line(
            Vector {
                x: coords.x + (w as f32 * 15.5) as i32,
                y: coords.y,
            },
            Vector {
                x: coords.x + (w as f32 * 15.5) as i32,
                y: coords.y + coords.h,
            },
            highlight::Color::Link("lineNumberSplit".to_string()),
        )?;

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
        "LogView".to_string()
    }

    fn set_focused(&mut self, _child: &Box<Buffer>) -> bool {
        true
    }

    fn close(&mut self, _lsp: &mut lsp::LSP) -> CloseKind {
        CloseKind::This
    }
}
