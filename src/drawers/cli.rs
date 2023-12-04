use crate::drawer::*;
use crate::event as ev;
use crate::highlight;
use crate::math::{Rect, Vector};
use crate::status::Status;
use crossterm::queue;
use crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, event, execute, style, terminal, QueueableCommand};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{stdout, BufWriter, Stdout, Write};
use std::time::Duration;

pub struct CliHandle<'a> {
    pub stdout: RefCell<BufWriter<Stdout>>,
    pub colors: &'a HashMap<String, highlight::Color>,
}

impl Handle for CliHandle<'_> {
    fn end(&self) -> std::io::Result<()> {
        let mut tmp = self.stdout.borrow_mut();
        queue!(tmp, EndSynchronizedUpdate,)?;
        tmp.flush()?;

        Ok(())
    }

    fn render_text(&self, lines: Vec<Line>, bounds: Rect, mode: TextMode) -> std::io::Result<()> {
        let mut tmp = self.stdout.borrow_mut();

        let mut idx = 0;
        for l in lines {
            if idx > bounds.h {
                break;
            }

            let mut line = truncate(&l.chars, bounds.w as usize).to_string();
            if line.len() != l.chars.len() {
                let mut tmp = line.chars();
                tmp.next_back();
                line = (&tmp.as_str()).to_string() + ">";
            }

            let mut chars = line.chars();

            let mut last = highlight::Color::Base16(0);
            let mut text = "".to_string();
            let mut x = bounds.x;

            for color in &l.colors[0..line.len()] {
                if last != *color {
                    queue!(
                        tmp,
                        cursor::MoveTo(x as u16, bounds.y as u16 + idx as u16,),
                        style::SetForegroundColor({
                            let last = highlight::get_color(self.colors, last);
                            match last {
                                Some(highlight::Color::Hex { r, g, b }) => {
                                    style::Color::Rgb { r, g, b }
                                }
                                _ => style::Color::White,
                            }
                        }),
                        style::Print(&text)
                    )?;
                    last = color.clone();
                    x += text.len() as i32;
                    text = "".to_string();
                    text.push(chars.next().unwrap());
                } else {
                    text.push(chars.next().unwrap());
                }
            }
            queue!(
                tmp,
                cursor::MoveTo(x as u16, bounds.y as u16 + idx as u16,),
                style::SetForegroundColor({
                    let last = highlight::get_color(self.colors, last);
                    match last {
                        Some(highlight::Color::Hex { r, g, b }) => style::Color::Rgb { r, g, b },
                        _ => style::Color::White,
                    }
                }),
                style::Print(text),
                style::ResetColor,
            )?;

            idx += 1;
        }

        Ok(())
    }

    fn render_line(&self, start: Vector, end: Vector) -> std::io::Result<()> {
        let dir = if start.x < end.x {
            Vector { x: 1, y: 0 }
        } else if start.x > end.x {
            Vector { x: -1, y: 0 }
        } else if start.y < end.y {
            Vector { y: 1, x: 0 }
        } else if start.y > end.y {
            Vector { y: -1, x: 0 }
        } else {
            return Ok(());
        };

        let mut pos = start;
        let mut tmp = self.stdout.borrow_mut();

        queue!(tmp, style::SetAttribute(style::Attribute::Reverse))?;

        while pos != end {
            queue!(
                tmp,
                cursor::MoveTo(pos.x as u16, pos.y as u16),
                style::Print(" "),
            )?;

            pos.x += dir.x;
            pos.y += dir.y;
        }
        queue!(tmp, style::SetAttribute(style::Attribute::Reset))?;

        Ok(())
    }

    fn render_cursor(&self, cur: CursorData) -> std::io::Result<()> {
        let mut tmp = self.stdout.borrow_mut();

        match cur {
            CursorData::Show { pos, kind, .. } => {
                queue!(
                    tmp,
                    cursor::MoveTo(pos.x as u16, pos.y as u16),
                    match kind {
                        CursorStyle::Block => cursor::SetCursorStyle::SteadyBlock,
                        CursorStyle::Bar => cursor::SetCursorStyle::BlinkingBar,
                    }
                )?;
            }
            CursorData::Hidden => {}
        }

        Ok(())
    }

    fn render_status(&self, st: Status, size: Rect) -> std::io::Result<()> {
        let total = size.w as usize;
        let y = size.y;

        let left = truncate(&st.left, total);
        let xl = left.len();

        let mut xr = total;

        let rr: String = st.right.chars().rev().collect();
        let right: String = truncate(&rr, total - xl).chars().rev().collect();
        xr -= right.len();

        queue!(
            self.stdout.borrow_mut(),
            cursor::MoveTo(0 as u16, y as u16),
            style::SetAttribute(style::Attribute::Reverse),
            style::Print(left),
            style::Print(" ".repeat(xr - xl)),
            style::Print(right),
            style::SetAttribute(style::Attribute::Reset),
        )?;

        Ok(())
    }

    fn get_char_size(&self) -> std::io::Result<Vector> {
        Ok(Vector { x: 1, y: 1 })
    }
}

pub struct CliDrawer {
    pub stdout: Stdout,
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

impl Drawer for CliDrawer {
    fn init(&mut self) -> std::io::Result<()> {
        execute!(self.stdout, EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        Ok(())
    }

    fn deinit(&mut self) -> std::io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(self.stdout, LeaveAlternateScreen)?;

        Ok(())
    }

    fn begin<'a>(
        &mut self,
        colors: &'a HashMap<String, highlight::Color>,
    ) -> std::io::Result<Box<dyn Handle + 'a>> {
        queue!(
            self.stdout,
            terminal::Clear(terminal::ClearType::All),
            BeginSynchronizedUpdate,
        )?;

        Ok(Box::new(CliHandle {
            stdout: RefCell::new(BufWriter::new(stdout())),
            colors,
        }))
    }

    fn get_size(&self) -> std::io::Result<Vector> {
        let size = terminal::size()?;

        Ok(Vector {
            x: size.0 as i32,
            y: size.1 as i32 - 1, // room for status
        })
    }

    fn get_events(&mut self) -> Vec<ev::Event> {
        if event::poll(Duration::from_millis(500)).unwrap() {
            match event::read().unwrap() {
                event::Event::Key(event::KeyEvent {
                    kind,
                    code,
                    modifiers: mods,
                    ..
                }) if kind != event::KeyEventKind::Release => {
                    let mut mods = ev::Mods {
                        ctrl: mods.contains(event::KeyModifiers::CONTROL),
                        alt: mods.contains(event::KeyModifiers::ALT),
                        shift: mods.contains(event::KeyModifiers::SHIFT),
                    };

                    match code {
                        event::KeyCode::Char(c) => {
                            if c == 'c' && mods.ctrl {
                                return vec![ev::Event::Quit];
                            }
                            if ":".contains(c) {
                                mods.shift = true;
                            }
                            return vec![ev::Event::Key(mods, c)];
                        }
                        event::KeyCode::Up => return vec![ev::Event::Nav(mods, ev::Nav::Up)],
                        event::KeyCode::Down => return vec![ev::Event::Nav(mods, ev::Nav::Down)],
                        event::KeyCode::Left => return vec![ev::Event::Nav(mods, ev::Nav::Left)],
                        event::KeyCode::Right => return vec![ev::Event::Nav(mods, ev::Nav::Right)],
                        event::KeyCode::Esc => return vec![ev::Event::Nav(mods, ev::Nav::Escape)],
                        event::KeyCode::Enter => return vec![ev::Event::Nav(mods, ev::Nav::Enter)],
                        event::KeyCode::Backspace => {
                            return vec![ev::Event::Nav(mods, ev::Nav::BackSpace)]
                        }
                        _ => {}
                    }
                }
                //match (mods, code) {
                //    (event::KeyModifiers::CONTROL, event::KeyCode::Char(c)) if c == 'c' => {
                //        break;
                //    }
                //    (event::KeyModifiers::CONTROL, event::KeyCode::Char(c)) if c == 't' => {
                //        bu = Box::new(TabbedBuffer {
                //            tabs: vec![bu],
                //            active: 0,
                //        });
                //    }
                //    (event::KeyModifiers::CONTROL, event::KeyCode::Char(c)) if c == 'l' => {
                //        bu = Box::new(SplitBuffer {
                //            a: bu,
                //            b: Box::new(EmptyBuffer {}),
                //            split_dir: SplitDir::Horizontal,
                //            a_active: false,
                //            split: Measurement::Percent(0.5),
                //        });
                //    }
                //    (event::KeyModifiers::CONTROL, event::KeyCode::Char(c)) if c == 'p' => {
                //        bu = Box::new(SplitBuffer {
                //            a: bu,
                //            b: Box::new(EmptyBuffer {}),
                //            split_dir: SplitDir::Vertical,
                //            a_active: false,
                //            split: Measurement::Percent(0.5),
                //        });
                //    }
                //    (event::KeyModifiers::CONTROL, event::KeyCode::Char(c)) if c == 'o' => {
                //        let Some(filename) =
                //            status.prompt("Enter a file path".to_string(), &mut drawer, &mut bu)?
                //        else {
                //            continue;
                //        };

                //        let adds: Box<dyn Buffer> = Box::new(FileBuffer {
                //            filename,
                //            cached: false,
                //            data: Vec::new(),
                //            pos: Vector { x: 0, y: 0 },
                //            scroll: 0,
                //            mode: FileMode::Normal,
                //        });
                //        if bu.set_focused(&adds) {
                //            bu = adds;
                //        }
                //    }
                //    _ => bu.key_press(code, mods),
                //},
                _ => {}
            }
        }
        vec![]
    }
}
