use crate::buffer::*;
use crate::drawer;
use crate::event;
use crate::lsp;
use crate::math::*;
use crate::CloseKind;

#[derive(Clone)]
pub struct EmptyBuffer {}

impl BufferFuncs for EmptyBuffer {
    fn update(&mut self, _size: Vector) {}

    fn draw_conts(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        handle.render_text(
            vec![
                drawer::Line::Image {
                    path: "lol.png".to_string(),
                    height: 128,
                },
                create_line("        EMPTY BUFFER        ".to_string()),
                create_line("Press Ctrl-O to open a file!".to_string()),
            ],
            coords,
            drawer::TextMode::Center,
        )?;

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
