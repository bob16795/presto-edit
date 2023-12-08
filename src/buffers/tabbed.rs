use crate::buffer::*;
use crate::drawer;
use crate::drawer::Drawable;
use crate::event;
use crate::lsp;
use crate::math::*;
use crate::EmptyBuffer;

#[derive(Clone)]
pub struct TabbedBuffer {
    pub tabs: Vec<Box<Buffer>>,
    pub active: usize,
    pub char_size: Vector,
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

    fn get_cursor(&mut self, size: Vector, char_size: Vector) -> drawer::CursorData {
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
