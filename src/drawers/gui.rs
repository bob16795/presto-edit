use crate::drawer;
use crate::event as ev;
use crate::highlight;
use crate::math::{Rect, Vector};
use crate::status::Status;
use raylib::core::*;
use raylib::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct GuiHandle<'a, 'b, 'c, 'd> {
    pub h: RefCell<RaylibDrawHandle<'a>>,
    pub font: &'b Font,
    pub cursor: &'c RefCell<[Vector2; 4]>,
    pub cursor_targ: &'c RefCell<[Vector2; 4]>,
    pub cursor_t: &'c RefCell<[f32; 4]>,
    pub colors: &'d HashMap<String, highlight::Color>,
}

const TRAIL_SIZE: f32 = 10.0;
const FONT_SIZE: f32 = 20.0;

#[allow(dead_code)]
pub fn ease_out_expo(t: f32) -> f32 {
    if (t - 1.0).abs() < std::f32::EPSILON {
        1.0
    } else {
        1.0 - 2.0f32.powf(-10.0 * t)
    }
}

fn lerp_point(
    point: &mut Vector2,
    old_targ: &mut Vector2,
    targ: Vector2,
    center: Vector2,
    t: &mut f32,
) -> Vector2 {
    if *old_targ != targ {
        *point = point.lerp(*old_targ, ease_out_expo(*t));
        *t = 0.0;
        *old_targ = targ;
    }
    // Check first if animation's over
    if (*t - 1.0).abs() < std::f32::EPSILON {
        return targ;
    }

    let trav_dir = {
        let mut d = targ - *point;
        d.normalize();
        d
    };

    let corner_dir = {
        let mut d = center;
        d.normalize();
        d
    };

    let direction_alignment = trav_dir.dot(corner_dir);

    if (*t - 1.0).abs() < std::f32::EPSILON {
        // We are at destination, move t out of 0-1 range to stop the animation
        *t = 2.0;
        *point = targ;
    } else {
        let corner_dt = lerp(
            1.0,
            (1.0 - TRAIL_SIZE).max(0.0).min(1.0),
            -direction_alignment,
        )
        .clamp(0.1, 1.0)
            * 0.2;
        *t = (*t + corner_dt / (0.5)).min(1.0);
    }

    point.lerp(targ, ease_out_expo(*t))
}

impl GuiHandle<'_, '_, '_, '_> {
    fn get_color(&self, name: String) -> Color {
        match highlight::get_color(self.colors, highlight::Color::Link(name)) {
            Some(highlight::Color::Hex { r, g, b }) => Color { r, g, b, a: 255 },
            _ => Color {
                r: 255,
                g: 0,
                b: 255,
                a: 255,
            },
        }
    }
}

impl drawer::Handle for GuiHandle<'_, '_, '_, '_> {
    fn end(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn render_text(
        &self,
        lines: Vec<drawer::Line>,
        bounds: Rect,
        mode: drawer::TextMode,
    ) -> std::io::Result<()> {
        match mode {
            drawer::TextMode::Center => {
                let mut tmp = self.h.borrow_mut();

                let sizey = FONT_SIZE * lines.len() as f32;

                let mut y = (bounds.y as f32 + (bounds.h as f32 - sizey) / 2.0) as i32;
                let mut tmp = tmp.begin_scissor_mode(bounds.x, bounds.y, bounds.w, bounds.h);

                let mut line = 0;

                while y < bounds.y + bounds.h {
                    if line >= lines.len() {
                        break;
                    }

                    let size = measure_text_ex(self.font, &lines[line].chars, FONT_SIZE, 0.0).x;

                    tmp.draw_text_ex(
                        self.font,
                        &lines[line].chars,
                        Vector2 {
                            x: ((bounds.x as f32 + (bounds.w as f32 - size) / 2.0) as i32) as f32,
                            y: y as f32,
                        },
                        FONT_SIZE,
                        0.0,
                        self.get_color("fg".to_string()),
                    );
                    line += 1;
                    y += FONT_SIZE as i32;
                }

                Ok(())
            }
            drawer::TextMode::Lines => {
                let mut tmp = self.h.borrow_mut();

                let mut y = bounds.y;
                let mut tmp = tmp.begin_scissor_mode(bounds.x, bounds.y, bounds.w, bounds.h);

                let mut line = 0;

                while y < bounds.y + bounds.h {
                    if line >= lines.len() {
                        break;
                    }

                    let mut chars = lines[line].chars.chars();

                    let mut last_color = highlight::Color::Base16(0);
                    let mut text = "".to_string();
                    let mut x = bounds.x;

                    for color in &lines[line].colors[0..lines[line].chars.len()] {
                        if last_color != *color {
                            tmp.draw_text_ex(
                                self.font,
                                &text,
                                Vector2 {
                                    x: x as f32,
                                    y: y as f32,
                                },
                                FONT_SIZE,
                                0.0,
                                match highlight::get_color(self.colors, last_color) {
                                    Some(highlight::Color::Hex { r, g, b }) => {
                                        Color { r, g, b, a: 255 }
                                    }
                                    _ => Color {
                                        r: 255,
                                        g: 0,
                                        b: 255,
                                        a: 255,
                                    },
                                },
                            );
                            last_color = color.clone();
                            let size = measure_text_ex(self.font, &text, FONT_SIZE, 0.0).x;
                            x += size as i32;
                            text = "".to_string();
                            text.push(chars.next().unwrap());
                        } else {
                            text.push(chars.next().unwrap());
                        }
                    }
                    tmp.draw_text_ex(
                        self.font,
                        &text,
                        Vector2 {
                            x: x as f32,
                            y: y as f32,
                        },
                        FONT_SIZE,
                        0.0,
                        match highlight::get_color(self.colors, last_color) {
                            Some(highlight::Color::Hex { r, g, b }) => Color { r, g, b, a: 255 },
                            _ => Color {
                                r: 255,
                                g: 0,
                                b: 255,
                                a: 255,
                            },
                        },
                    );

                    line += 1;
                    y += FONT_SIZE as i32;
                }

                Ok(())
            }
        }
    }

    fn render_line(&self, start: Vector, end: Vector) -> std::io::Result<()> {
        let mut tmp = self.h.borrow_mut();

        tmp.draw_line_ex(
            Vector2 {
                x: start.x as f32,
                y: start.y as f32,
            },
            Vector2 {
                x: end.x as f32,
                y: end.y as f32,
            },
            2.0,
            self.get_color("fg".to_string()),
        );

        Ok(())
    }

    fn render_cursor(&self, cur: drawer::CursorData) -> std::io::Result<()> {
        match cur {
            drawer::CursorData::Show { pos, size, kind } => {
                let cursor: &mut [Vector2; 4] = &mut self.cursor.borrow_mut();
                let cursor_targ: &mut [Vector2; 4] = &mut self.cursor_targ.borrow_mut();
                let cursor_t: &mut [f32; 4] = &mut self.cursor_t.borrow_mut();

                let mut out_cursor = [Vector2 { x: 0.0, y: 0.0 }; 4];
                let mut size = size;
                if kind == drawer::CursorStyle::Bar {
                    size.x /= 5;
                }

                out_cursor[0] = lerp_point(
                    &mut cursor[0],
                    &mut cursor_targ[0],
                    Vector2 {
                        x: (pos.x + size.x) as f32,
                        y: (pos.y) as f32,
                    },
                    Vector2 {
                        x: (0.5) as f32,
                        y: (-0.5) as f32,
                    },
                    &mut cursor_t[0],
                );

                out_cursor[1] = lerp_point(
                    &mut cursor[1],
                    &mut cursor_targ[1],
                    Vector2 {
                        x: (pos.x) as f32,
                        y: (pos.y) as f32,
                    },
                    Vector2 {
                        x: (-0.5) as f32,
                        y: (-0.5) as f32,
                    },
                    &mut cursor_t[1],
                );

                out_cursor[2] = lerp_point(
                    &mut cursor[2],
                    &mut cursor_targ[2],
                    Vector2 {
                        x: (pos.x + size.x) as f32,
                        y: (pos.y + size.y) as f32,
                    },
                    Vector2 {
                        x: (0.5) as f32,
                        y: (0.5) as f32,
                    },
                    &mut cursor_t[2],
                );

                out_cursor[3] = lerp_point(
                    &mut cursor[3],
                    &mut cursor_targ[3],
                    Vector2 {
                        x: (pos.x) as f32,
                        y: (pos.y + size.y) as f32,
                    },
                    Vector2 {
                        x: (-0.5) as f32,
                        y: (0.5) as f32,
                    },
                    &mut cursor_t[3],
                );

                let mut tmp = self.h.borrow_mut();
                tmp.draw_triangle_strip(
                    &out_cursor,
                    self.get_color("cursor".to_string()).fade(0.75),
                );
            }
            drawer::CursorData::Hidden => {}
        }

        Ok(())
    }

    fn render_status(&self, st: Status, coords: Rect) -> std::io::Result<()> {
        let mut tmp = self.h.borrow_mut();

        tmp.draw_rectangle(
            0,
            coords.y as i32,
            coords.w,
            FONT_SIZE as i32 + 5,
            self.get_color("statusBg".to_string()),
        );

        tmp.draw_text_ex(
            self.font,
            &st.left,
            Vector2 {
                x: coords.x as f32,
                y: coords.y as f32,
            },
            FONT_SIZE,
            0.0,
            self.get_color("statusFg".to_string()),
        );

        let size = measure_text_ex(self.font, &st.right, FONT_SIZE, 0.0).x;
        let pos = coords.w as f32 - size;

        tmp.draw_text_ex(
            self.font,
            &st.right,
            Vector2 {
                x: pos as f32,
                y: coords.y as f32,
            },
            FONT_SIZE,
            0.0,
            self.get_color("statusFg".to_string()),
        );

        Ok(())
    }

    fn get_char_size(&self) -> std::io::Result<Vector> {
        Ok(Vector {
            x: measure_text_ex(self.font, " ", FONT_SIZE, 0.0).x as i32,
            y: measure_text_ex(self.font, " ", FONT_SIZE, 0.0).y as i32,
        })
    }
}

const MAX_TIMEOUT: i32 = 10;
const MIN_TIMEOUT: i32 = 0;
static mut TIMEOUT: i32 = 0;
static mut LAST_TIMEOUT: i32 = MAX_TIMEOUT;
static mut LAST: KeyboardKey = KeyboardKey::KEY_NULL;

fn is_key_pressed_repeat(rl: &RaylibHandle, key: KeyboardKey) -> bool {
    unsafe {
        if rl.is_key_pressed(key) {
            LAST = key;
            TIMEOUT = MAX_TIMEOUT;
            LAST_TIMEOUT = MAX_TIMEOUT;
            return true;
        } else if rl.is_key_down(key) {
            if TIMEOUT <= 0 {
                LAST_TIMEOUT = MIN_TIMEOUT.max(LAST_TIMEOUT - 2);
                TIMEOUT = LAST_TIMEOUT;
                return true;
            } else {
                if LAST == key {
                    TIMEOUT -= 1;
                }
            }
        } else {
            if LAST == key {
                LAST = KeyboardKey::KEY_NULL;
            }
        }
    }
    return false;
}

pub struct GuiDrawer {
    pub rl: RaylibHandle,
    pub thread: RaylibThread,
    pub font: Option<Font>,
    pub cursor: RefCell<[Vector2; 4]>,
    pub cursor_targ: RefCell<[Vector2; 4]>,
    pub cursor_t: RefCell<[f32; 4]>,
}

impl drawer::Drawer for GuiDrawer {
    fn init(&mut self) -> std::io::Result<()> {
        self.rl.set_exit_key(None);
        self.font = Some(self.rl.load_font(&self.thread, "font.ttf").unwrap());
        Ok(())
    }

    fn deinit(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn begin<'a>(
        &'a mut self,
        colors: &'a HashMap<String, highlight::Color>,
    ) -> std::io::Result<Box<dyn drawer::Handle + 'a>> {
        let h = self.rl.begin_drawing(&self.thread);
        let result = GuiHandle {
            h: RefCell::new(h),
            font: self.font.as_ref().unwrap(),
            cursor: &self.cursor,
            cursor_targ: &self.cursor_targ,
            cursor_t: &self.cursor_t,
            colors,
        };

        result
            .h
            .borrow_mut()
            .clear_background(result.get_color("bg".to_string()));

        Ok(Box::new(result))
    }

    fn get_size(&self) -> std::io::Result<Vector> {
        Ok(Vector {
            x: self.rl.get_screen_width(),
            y: self.rl.get_screen_height() - 20,
        })
    }

    fn get_events(&mut self) -> Vec<ev::Event> {
        if self.rl.window_should_close() {
            return vec![ev::Event::Quit];
        }

        let mut result = Vec::new();

        let mods = &mut ev::Mods {
            ctrl: self.rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
                || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL),
            shift: self.rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
                || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT),
            alt: self.rl.is_key_down(KeyboardKey::KEY_LEFT_ALT)
                || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT),
        };

        for (k, v) in [
            (KeyboardKey::KEY_UP, ev::Nav::Up),
            (KeyboardKey::KEY_DOWN, ev::Nav::Down),
            (KeyboardKey::KEY_LEFT, ev::Nav::Left),
            (KeyboardKey::KEY_RIGHT, ev::Nav::Right),
            (KeyboardKey::KEY_ENTER, ev::Nav::Enter),
            (KeyboardKey::KEY_ESCAPE, ev::Nav::Escape),
            (KeyboardKey::KEY_BACKSPACE, ev::Nav::BackSpace),
        ] {
            if is_key_pressed_repeat(&self.rl, k) {
                result.push(ev::Event::Nav(mods.clone(), v));
            }
        }

        for (s, k, v) in [
            (false, KeyboardKey::KEY_SLASH, '/'),
            (false, KeyboardKey::KEY_PERIOD, '.'),
            (false, KeyboardKey::KEY_SEMICOLON, ';'),
            (true, KeyboardKey::KEY_SEMICOLON, ':'),
            (false, KeyboardKey::KEY_SPACE, ' '),
            (false, KeyboardKey::KEY_ONE, '1'),
            (false, KeyboardKey::KEY_TWO, '2'),
            (false, KeyboardKey::KEY_THREE, '3'),
            (false, KeyboardKey::KEY_FOUR, '4'),
            (false, KeyboardKey::KEY_FIVE, '5'),
            (false, KeyboardKey::KEY_SIX, '6'),
            (false, KeyboardKey::KEY_SEVEN, '7'),
            (false, KeyboardKey::KEY_EIGHT, '8'),
            (false, KeyboardKey::KEY_NINE, '9'),
            (false, KeyboardKey::KEY_ZERO, '0'),
            (true, KeyboardKey::KEY_ONE, '!'),
            (true, KeyboardKey::KEY_TWO, '@'),
            (true, KeyboardKey::KEY_THREE, '#'),
            (true, KeyboardKey::KEY_FOUR, '$'),
            (true, KeyboardKey::KEY_FIVE, '%'),
            (true, KeyboardKey::KEY_SIX, '^'),
            (true, KeyboardKey::KEY_SEVEN, '&'),
            (true, KeyboardKey::KEY_EIGHT, '*'),
            (true, KeyboardKey::KEY_NINE, '('),
            (true, KeyboardKey::KEY_ZERO, ')'),
        ] {
            if is_key_pressed_repeat(&self.rl, k) && s == mods.shift {
                let os = mods.shift;
                mods.shift = false;
                result.push(ev::Event::Key(mods.clone(), v));
                mods.shift = os;
            }
        }

        let mut ch = 'a';
        for k in KeyboardKey::KEY_A as u32..KeyboardKey::KEY_Z as u32 {
            if is_key_pressed_repeat(&self.rl, unsafe { std::mem::transmute(k as u32) }) {
                result.push(ev::Event::Key(mods.clone(), ch));
            }

            ch = (ch as u8 + 1) as char;
        }

        result
    }
}
