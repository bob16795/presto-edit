use crate::drawer;
use crate::drawers::helpers;
use crate::event as ev;
use crate::highlight;
use crate::math::{Rect, Vector};
use crate::status::Status;
use freetype::face::LoadFlag;
use freetype::*;
use glfw;
use glfw::Context;
use image::io::Reader as ImageReader;
use ogl33::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;

const TRAIL_SIZE: f32 = 10.0;
const FONT_SIZE: u32 = 32;
const SCALE: f32 = 0.75;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    fn lerp(&self, b: Vector2, pc: f32) -> Self {
        Vector2 {
            x: self.x + (b.x - self.x) * pc,
            y: self.y + (b.y - self.y) * pc,
        }
    }

    fn normalize(&mut self) {
        let mag = (self.x * self.x + self.y * self.y).sqrt();
        self.x /= mag;
        self.y /= mag;
    }
}

#[derive(Debug)]
pub struct CharData {
    tex: i32,
    tx: f32,
    ty: f32,
    tw: f32,
    th: f32,
    bearing: Vector,
    advance: i64,
    size: Vector,
}

pub struct GlFont {
    size: i32,
    textures: Vec<u32>,
    chars: HashMap<char, CharData>,
    vao: u32,
    vbo: u32,
    program: helpers::ShaderProgram,
}

const FONT_TEX_SIZE: i32 = 1024;
const FONT_VERT_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec4 vertex; // <vec2 pos, vec2 tex>
out vec2 TexCoords;

uniform int width;
uniform int height;

uniform mat4 projection;

void main()
{
    gl_Position = vec4((vertex.x / width * 2) - 1, ((vertex.y / height * 2)- 1) * -1 , 0.0, 1.0);
    TexCoords = vertex.zw;
}"#;

const FONT_FRAG_SHADER: &str = r#"
#version 330 core
in vec2 TexCoords;
out vec4 out_color;

uniform sampler2D tex;
uniform vec4 color;

void main()
{
    float dist = (0.5 - texture(tex, TexCoords).r) * 2;
    vec2 duv = fwidth(TexCoords);

    float dtex = length(duv * 48);
    
    float pixelDist = dist / dtex;

    float alpha = 0.5 - clamp(pixelDist, -1, 1) * 0.5;
    out_color = color * vec4(1, 1, 1, alpha);
}  
"#;

const SOLID_VERT_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec4 vertex; // <vec2 pos, vec2 tex>
out vec2 TexCoords;

uniform int width;
uniform int height;

uniform mat4 projection;

void main()
{
    gl_Position = vec4((vertex.x / width * 2) - 1, ((vertex.y / height * 2)- 1) * -1 , 0.0, 1.0);
    TexCoords = vertex.zw;
}"#;

const SOLID_FRAG_SHADER: &str = r#"
#version 330 core
in vec2 TexCoords;
out vec4 out_color;

uniform sampler2D tex;
uniform vec4 color;
uniform int image;

void main()
{    
    out_color = texture(tex, TexCoords); 

    if (image == 0) {
        out_color = color;
    }
}  
"#;

impl GlFont {
    pub fn new(path: &str) -> Self {
        let lib = Library::init().unwrap();
        let face = lib.new_face(path, 0).unwrap();

        face.set_pixel_sizes(0, FONT_SIZE).unwrap();
        let mut textures: Vec<u32> = Vec::new();
        let mut chars = HashMap::new();

        textures.push(0);

        unsafe {
            glGenTextures(1, textures.last_mut().unwrap());
            glBindTexture(GL_TEXTURE_2D, *textures.last().unwrap());
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RGBA as i32,
                FONT_TEX_SIZE,
                FONT_TEX_SIZE,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                0 as *const _,
            );
        }

        let mut height = 0;

        let mut ax = 0;
        let mut ay = 0;
        let mut row_height = 0;

        for idx in 0..2560 {
            if face.load_char(idx, LoadFlag::RENDER).is_err() {
                continue;
            }
            if face.glyph().render_glyph(RenderMode::Sdf).is_err() {
                continue;
            }

            let mut x = ax;
            let mut y = ay;

            if face.glyph().bitmap().width() != 0 && face.glyph().bitmap().rows() != 0 {
                ax += face.glyph().bitmap().width() + 1;
                if ax >= FONT_TEX_SIZE {
                    x = 0;
                    ax = face.glyph().bitmap().width() + 1;
                    ay += row_height;
                    row_height = face.glyph().bitmap().rows() + 1;
                }

                if ay + face.glyph().bitmap().rows() + 1 >= FONT_TEX_SIZE {
                    y = 0;
                    ax = face.glyph().bitmap().width() + 1;
                    ay = 0;
                    x = 0;

                    textures.push(0);
                    unsafe {
                        glGenTextures(1, textures.last_mut().unwrap());
                        glBindTexture(GL_TEXTURE_2D, *textures.last().unwrap());
                        glTexImage2D(
                            GL_TEXTURE_2D,
                            0,
                            GL_RGBA as i32,
                            FONT_TEX_SIZE,
                            FONT_TEX_SIZE,
                            0,
                            GL_RGBA,
                            GL_UNSIGNED_BYTE,
                            0 as *const _,
                        );
                    }
                }

                row_height = row_height.max(face.glyph().bitmap().rows() + 1);
                height = height.max(face.glyph().bitmap().rows());

                unsafe {
                    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
                    glTexSubImage2D(
                        GL_TEXTURE_2D,
                        0,
                        x,
                        y,
                        face.glyph().bitmap().width(),
                        face.glyph().bitmap().rows(),
                        GL_RED,
                        GL_UNSIGNED_BYTE,
                        face.glyph().bitmap().buffer().as_ptr() as *const _,
                    );
                }
            }

            chars.insert(
                char::from_u32(idx as u32).unwrap(),
                CharData {
                    size: Vector {
                        x: face.glyph().bitmap().width(),
                        y: face.glyph().bitmap().rows(),
                    },
                    bearing: Vector {
                        x: face.glyph().bitmap_left(),
                        y: face.glyph().bitmap_top(),
                    },
                    advance: face.glyph().advance().x,
                    tex: (textures.len() - 1) as i32,
                    tx: x as f32 / FONT_TEX_SIZE as f32,
                    ty: y as f32 / FONT_TEX_SIZE as f32,
                    tw: face.glyph().bitmap().width() as f32 / FONT_TEX_SIZE as f32,
                    th: face.glyph().bitmap().rows() as f32 / FONT_TEX_SIZE as f32,
                },
            );
        }

        let mut vbo: u32 = 0;
        let mut vao: u32 = 0;
        unsafe {
            glGenVertexArrays(1, &mut vao);
            glGenBuffers(1, &mut vbo);
            glBindVertexArray(vao);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            glBufferData(GL_ARRAY_BUFFER, 4 * 6 * 4, 0 as *const _, GL_DYNAMIC_DRAW);
            glEnableVertexAttribArray(0);
            glVertexAttribPointer(0, 4, GL_FLOAT, GL_FALSE, 4 * 4, 0 as *const _);
            glBindBuffer(GL_ARRAY_BUFFER, 0);
            glBindVertexArray(0);
        }

        let program =
            helpers::ShaderProgram::from_vert_frag(FONT_VERT_SHADER, FONT_FRAG_SHADER).unwrap();

        for tex in &mut textures {
            unsafe {
                glBindTexture(GL_TEXTURE_2D, *tex);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
                glTexParameteri(
                    GL_TEXTURE_2D,
                    GL_TEXTURE_MIN_FILTER,
                    GL_LINEAR_MIPMAP_LINEAR as i32,
                );
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);

                glGenerateMipmap(GL_TEXTURE_2D);
            }
        }

        GlFont {
            size: FONT_SIZE as i32,
            textures,
            chars,
            vao,
            vbo,
            program,
        }
    }

    fn render(&self, x: i32, y: i32, text: String, scale: f32, colors: Vec<highlight::Color>) {
        let mut pos = Vector {
            x,
            y: y + (self.size as f32 * scale) as i32,
        };

        unsafe {
            glActiveTexture(GL_TEXTURE0);
            glBindVertexArray(self.vao);
        }

        let mut idx = 0;

        for c in text.chars() {
            if !self.chars.contains_key(&c) {
                continue;
            };

            let ch = self.chars.get(&c).unwrap();
            let w = ch.size.x as f32 * scale;
            let h = ch.size.y as f32 * scale;
            let xpos = pos.x as f32 + ch.bearing.x as f32 * scale;
            let ypos = pos.y as f32 - ch.bearing.y as f32 * scale;

            let verts = [
                [xpos as f32, ypos as f32, ch.tx, ch.ty],
                [xpos as f32, (ypos + h) as f32, ch.tx, ch.ty + ch.th],
                [
                    (xpos + w) as f32,
                    (ypos + h) as f32,
                    ch.tx + ch.tw,
                    ch.ty + ch.th,
                ],
                [xpos as f32, ypos as f32, ch.tx, ch.ty],
                [
                    (xpos + w) as f32,
                    (ypos + h) as f32,
                    ch.tx + ch.tw,
                    ch.ty + ch.th,
                ],
                [(xpos + w) as f32, ypos as f32, ch.tx + ch.tw, ch.ty],
            ];

            self.program.use_program();

            unsafe {
                match colors.iter().nth(idx) {
                    Some(highlight::Color::Hex { r, g, b }) => self.program.set_uniform_color(
                        "color\0",
                        [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0, 1.0],
                    ),

                    Some(_) => self
                        .program
                        .set_uniform_color("color\0", [1.0, 0.0, 0.0, 1.0]),

                    _ => {}
                }

                glBindTexture(GL_TEXTURE_2D, self.textures[ch.tex as usize]);

                glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
                glBufferSubData(GL_ARRAY_BUFFER, 0, 4 * 6 * 4, (&verts).as_ptr() as *const _);
                glBindBuffer(GL_ARRAY_BUFFER, 0);

                // render quad
                glDrawArrays(GL_TRIANGLES, 0, 6);
            }

            idx += 1;

            pos.x += ((ch.advance >> 6) as f32 * scale) as i32;
        }
    }
}

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
        let mut d = Vector2 {
            x: targ.x - point.x,
            y: targ.y - point.y,
        };
        d.normalize();
        d
    };

    let corner_dir = {
        let mut d = center;
        d.normalize();
        d
    };

    let direction_alignment = trav_dir.x * corner_dir.x + trav_dir.y * corner_dir.y;

    if (*t - 1.0).abs() < std::f32::EPSILON {
        // We are at destination, move t out of 0-1 range to stop the animation
        *t = 2.0;
        *point = targ;
    } else {
        let corner_dt = (1.0
            + (((1.0 - TRAIL_SIZE).max(0.0).min(1.0) - 1.0) * -direction_alignment))
            .clamp(0.1, 1.0)
            * 0.1;
        *t = (*t + corner_dt / (0.5)).min(1.0);
    }

    point.lerp(targ, ease_out_expo(*t))
}

pub struct GlHandle<'a> {
    win: &'a RefCell<glfw::PWindow>,
    font: &'a RefCell<GlFont>,
    program: &'a RefCell<Option<helpers::ShaderProgram>>,
    cursor: &'a RefCell<[Vector2; 4]>,
    cursor_targ: &'a RefCell<[Vector2; 4]>,
    cursor_t: &'a RefCell<[f32; 4]>,
    colors: &'a HashMap<String, highlight::Color>,
    images: &'a RefCell<HashMap<String, (u32, Vector)>>,
    size: Vector2,
}

impl GlHandle<'_> {
    fn get_color(&self, name: String) -> highlight::Color {
        match highlight::get_color(self.colors, highlight::Color::Link(name)) {
            Some(highlight::Color::Hex { r, g, b }) => highlight::Color::Hex { r, g, b },
            _ => highlight::Color::Hex {
                r: 255,
                g: 0,
                b: 255,
            },
        }
    }
}

impl drawer::Handle for GlHandle<'_> {
    fn render_text(
        &self,
        lines: Vec<drawer::Line>,
        bounds: Rect,
        mode: drawer::TextMode,
    ) -> std::io::Result<()> {
        unsafe {
            glScissor(
                bounds.x,
                self.size.y as i32 - bounds.h - bounds.y,
                bounds.w,
                bounds.h,
            );
            glEnable(GL_SCISSOR_TEST);
        }

        match mode {
            drawer::TextMode::Lines => {
                let tmp_font = self.font.borrow_mut();

                let mut y = bounds.y as f32;
                for line in lines {
                    if y as i32 > bounds.y + bounds.h {
                        break;
                    }

                    match line {
                        drawer::Line::Image { path, height } => {
                            let images = self.images.borrow_mut();

                            if images.get(&path) == None {
                                let mut image: u32 = 0;

                                let img = ImageReader::open(path.clone())?.decode().unwrap();
                                let img = img.as_rgb8().unwrap();

                                unsafe {
                                    glGenTextures(1, &mut image);
                                    glBindTexture(GL_TEXTURE_2D, image);
                                    glTexImage2D(
                                        GL_TEXTURE_2D,
                                        0,
                                        GL_RGBA as i32,
                                        img.clone().width() as i32,
                                        img.height() as i32,
                                        0,
                                        GL_RGBA,
                                        GL_RGB8,
                                        img.as_raw().as_ptr() as *const _,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_WRAP_S,
                                        GL_CLAMP_TO_EDGE as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_WRAP_T,
                                        GL_CLAMP_TO_EDGE as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_MIN_FILTER,
                                        GL_NEAREST as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_MAG_FILTER,
                                        GL_NEAREST as i32,
                                    );
                                }
                            }

                            let ft = self.font.borrow_mut();

                            let verts = [
                                0.0, 0.0, 0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0,
                                0.0, 0.0, 0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0,
                            ];

                            println!("{:?}", verts);

                            unsafe {
                                let img = images.get(&path).unwrap();
                                glBindTexture(GL_TEXTURE_2D, img.0);

                                glBindVertexArray(ft.vao);
                                glBindBuffer(GL_ARRAY_BUFFER, ft.vbo);
                                glBufferSubData(
                                    GL_ARRAY_BUFFER,
                                    0,
                                    4 * 6 * 4,
                                    (&verts).as_ptr() as *const _,
                                );
                                glBindBuffer(GL_ARRAY_BUFFER, 0);

                                // render quad
                                glDrawArrays(GL_TRIANGLES, 0, 6);
                            }
                        }
                        drawer::Line::Text {
                            chars: line_chars,
                            colors: line_colors,
                        } => {
                            tmp_font.render(
                                bounds.x,
                                y as i32,
                                line_chars.clone(),
                                SCALE,
                                line_colors
                                    .iter()
                                    .map(|c| match c {
                                        highlight::Color::Link(c) => self.get_color(c.to_string()),
                                        _ => self.get_color("red".to_string()),
                                    })
                                    .collect(),
                            );

                            y += tmp_font.size as f32 * SCALE;
                        }
                    }
                }
            }
            drawer::TextMode::Center => {
                let cw = self.get_char_size()?.x;

                let tmp_font = self.font.borrow_mut();

                let mut sizey = 0.0;
                for l in &lines {
                    match l {
                        drawer::Line::Image { height, .. } => {
                            sizey += *height as f32;
                        }
                        drawer::Line::Text { .. } => {
                            sizey += FONT_SIZE as f32 * SCALE as f32;
                        }
                    }
                }

                let mut y = (bounds.y as f32 + (bounds.h as f32 - sizey) / 2.0) as f32;

                let images = &mut self.images.borrow_mut();

                for line in lines {
                    if y as i32 > bounds.y + bounds.h {
                        break;
                    }

                    match line {
                        drawer::Line::Image { path, height } => {
                            if images.get(&path) == None {
                                let mut image: u32 = 0;
                                let img;

                                if path.starts_with("!!") {
                                    match (path.as_str()) {
                                        "!!logo" => {
                                            img = ImageReader::new(Cursor::new(
                                                include_bytes!("../assets/logo.png").to_vec(),
                                            ));
                                        }
                                        _ => todo!(),
                                    }
                                } else {
                                    img =
                                        ImageReader::new(Cursor::new(std::fs::read(path.clone())?));
                                }

                                let img = img.with_guessed_format()?.decode().unwrap().to_rgba8();

                                unsafe {
                                    glGenTextures(1, &mut image);
                                    glBindTexture(GL_TEXTURE_2D, image);
                                    glTexImage2D(
                                        GL_TEXTURE_2D,
                                        0,
                                        GL_RGBA as i32,
                                        img.clone().width() as i32,
                                        img.clone().height() as i32,
                                        0,
                                        GL_RGBA,
                                        GL_UNSIGNED_BYTE,
                                        img.as_raw().as_ptr() as *const _,
                                    );

                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_WRAP_S,
                                        GL_CLAMP_TO_EDGE as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_WRAP_T,
                                        GL_CLAMP_TO_EDGE as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_MIN_FILTER,
                                        GL_NEAREST as i32,
                                    );
                                    glTexParameteri(
                                        GL_TEXTURE_2D,
                                        GL_TEXTURE_MAG_FILTER,
                                        GL_NEAREST as i32,
                                    );
                                }

                                let width = height as f32 / img.clone().height() as f32
                                    * img.clone().width() as f32;

                                images.insert(
                                    path.clone(),
                                    (
                                        image,
                                        Vector {
                                            x: width as i32,
                                            y: height as i32,
                                        },
                                    ),
                                );
                            }

                            let img = images.get(&path).unwrap();

                            let offset = Vector2 {
                                x: (bounds.w as f32 - img.1.x as f32) / 2.0,
                                y,
                            };

                            let verts: [f32; 4 * 6] = [
                                bounds.x as f32 + offset.x,
                                bounds.y as f32 + offset.y,
                                0.0,
                                0.0,
                                (bounds.x + img.1.x) as f32 + offset.x,
                                (bounds.y + img.1.y) as f32 + offset.y,
                                1.0,
                                1.0,
                                bounds.x as f32 + offset.x,
                                (bounds.y + img.1.y) as f32 + offset.y,
                                0.0,
                                1.0,
                                bounds.x as f32 + offset.x,
                                bounds.y as f32 + offset.y,
                                0.0,
                                0.0,
                                (bounds.x + img.1.x) as f32 + offset.x,
                                (bounds.y + img.1.y) as f32 + offset.y,
                                1.0,
                                1.0,
                                (bounds.x + img.1.x) as f32 + offset.x,
                                bounds.y as f32 + offset.y,
                                1.0,
                                0.0,
                            ];

                            let prg = self.program.clone();
                            let mut prg = prg.borrow_mut();
                            let prg = prg.as_mut().unwrap();
                            prg.use_program();
                            prg.set_uniform_int("image\0", 1);

                            unsafe {
                                glBindTexture(GL_TEXTURE_2D, img.0);

                                glBindVertexArray(tmp_font.vao);
                                glBindBuffer(GL_ARRAY_BUFFER, tmp_font.vbo);
                                glBufferSubData(
                                    GL_ARRAY_BUFFER,
                                    0,
                                    4 * 6 * 4,
                                    (&verts).as_ptr() as *const _,
                                );
                                glBindBuffer(GL_ARRAY_BUFFER, 0);

                                // render quad
                                glDrawArrays(GL_TRIANGLES, 0, 6);
                            }

                            prg.set_uniform_int("image\0", 0);

                            y += img.1.y as f32;
                        }
                        drawer::Line::Text {
                            chars: line_chars,
                            colors: line_colors,
                        } => {
                            let w = cw as f32 * line_chars.len() as f32;

                            tmp_font.render(
                                bounds.x + ((bounds.w - w as i32) / 2),
                                y as i32,
                                line_chars.clone(),
                                SCALE,
                                line_colors
                                    .iter()
                                    .map(|c| match c {
                                        highlight::Color::Link(c) => self.get_color(c.to_string()),
                                        _ => self.get_color("red".to_string()),
                                    })
                                    .collect(),
                            );

                            y += tmp_font.size as f32 * SCALE;
                        }
                    }
                }
            }
        }

        unsafe {
            glDisable(GL_SCISSOR_TEST);
        }

        Ok(())
    }

    fn render_line(
        &self,
        start: Vector,
        end: Vector,
        color: highlight::Color,
    ) -> std::io::Result<()> {
        let verts = [
            start.x as f32 - 1.0,
            start.y as f32 - 1.0,
            0.0,
            0.0,
            end.x as f32 + 1.0,
            end.y as f32 + 1.0,
            0.0,
            0.0,
            start.x as f32 - 1.0,
            end.y as f32 + 1.0,
            0.0,
            0.0,
            start.x as f32 - 1.0,
            start.y as f32 - 1.0,
            0.0,
            0.0,
            end.x as f32 + 1.0,
            end.y as f32 + 1.0,
            0.0,
            0.0,
            end.x as f32 + 1.0,
            start.y as f32 - 1.0,
            0.0,
            0.0,
        ];

        let prg = self.program.clone();
        let mut prg = prg.borrow_mut();
        let prg = prg.as_mut().unwrap();
        prg.use_program();

        let ft = self.font.borrow_mut();

        let color = match color {
            highlight::Color::Link(l) => self.get_color(l),
            c => c,
        };

        if let highlight::Color::Hex { r, g, b } = color {
            prg.set_uniform_color(
                "color\0",
                [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
            );
        }

        unsafe {
            glBindVertexArray(ft.vao);
            glBindBuffer(GL_ARRAY_BUFFER, ft.vbo);
            glBufferSubData(GL_ARRAY_BUFFER, 0, 4 * 6 * 4, (&verts).as_ptr() as *const _);
            glBindBuffer(GL_ARRAY_BUFFER, 0);

            // render quad
            glDrawArrays(GL_TRIANGLES, 0, 6);
        }

        Ok(())
    }

    fn render_rect(
        &self,
        start: Vector,
        size: Vector,
        color: highlight::Color,
    ) -> std::io::Result<()> {
        self.render_line(
            Vector {
                x: start.x + 1,
                y: start.y + 1,
            },
            Vector {
                x: start.x + size.x - 2,
                y: start.y + size.y - 2,
            },
            color,
        )
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
                        y: (pos.y + size.y + 4) as f32,
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
                        y: (pos.y + size.y + 4) as f32,
                    },
                    Vector2 {
                        x: (-0.5) as f32,
                        y: (0.5) as f32,
                    },
                    &mut cursor_t[3],
                );

                let verts = [
                    out_cursor[0].x,
                    out_cursor[0].y,
                    0.0,
                    0.0,
                    out_cursor[1].x,
                    out_cursor[1].y,
                    0.0,
                    0.0,
                    out_cursor[2].x,
                    out_cursor[2].y,
                    0.0,
                    0.0,
                    out_cursor[2].x,
                    out_cursor[2].y,
                    0.0,
                    0.0,
                    out_cursor[1].x,
                    out_cursor[1].y,
                    0.0,
                    0.0,
                    out_cursor[3].x,
                    out_cursor[3].y,
                    0.0,
                    0.0,
                ];

                let prg = self.program.clone();
                let mut prg = prg.borrow_mut();
                let prg = prg.as_mut().unwrap();
                prg.use_program();

                let ft = self.font.borrow_mut();

                if let highlight::Color::Hex { r, g, b } = self.get_color("cursor".to_string()) {
                    prg.set_uniform_color(
                        "color\0",
                        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.75],
                    );
                }

                unsafe {
                    glBindVertexArray(ft.vao);
                    glBindBuffer(GL_ARRAY_BUFFER, ft.vbo);
                    glBufferSubData(GL_ARRAY_BUFFER, 0, 4 * 6 * 4, (&verts).as_ptr() as *const _);
                    glBindBuffer(GL_ARRAY_BUFFER, 0);

                    // render quad
                    glDrawArrays(GL_TRIANGLES, 0, 6);
                }
            }
            drawer::CursorData::Hidden => {}
        }

        Ok(())
    }

    fn render_status(&self, st: Status, _size: Rect) -> std::io::Result<()> {
        let verts = [
            0.0,
            self.size.y - self.get_char_size()?.y as f32 * 1.5,
            0.0,
            0.0,
            self.size.x,
            self.size.y,
            0.0,
            0.0,
            self.size.x,
            self.size.y - self.get_char_size()?.y as f32 * 1.5,
            0.0,
            0.0,
            0.0,
            self.size.y - self.get_char_size()?.y as f32 * 1.5,
            0.0,
            0.0,
            self.size.x,
            self.size.y,
            0.0,
            0.0,
            0.0,
            self.size.y,
            0.0,
            0.0,
        ];

        let prg = self.program.clone();
        let mut prg = prg.borrow_mut();
        let prg = prg.as_mut().unwrap();
        prg.use_program();

        if let highlight::Color::Hex { r, g, b } = self.get_color("statusBg".to_string()) {
            prg.set_uniform_color(
                "color\0",
                [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
            );
        }

        let h = self.get_char_size()?.y;
        let w = self.get_char_size()?.x as f32 * (st.right.len() + 1) as f32;
        let cw = self.get_char_size()?.x;

        let ft = self.font.borrow_mut();

        unsafe {
            glBindVertexArray(ft.vao);
            glBindBuffer(GL_ARRAY_BUFFER, ft.vbo);
            glBufferSubData(GL_ARRAY_BUFFER, 0, 4 * 6 * 4, (&verts).as_ptr() as *const _);
            glBindBuffer(GL_ARRAY_BUFFER, 0);

            // render quad
            glDrawArrays(GL_TRIANGLES, 0, 6);
        }

        ft.render(
            cw,
            (self.size.y - h as f32 * 1.5) as i32,
            st.left,
            SCALE,
            vec![self.get_color("statusFg".to_string())],
        );

        ft.render(
            (self.size.x - w) as i32,
            (self.size.y - h as f32 * 1.5) as i32,
            st.right,
            SCALE,
            vec![self.get_color("statusFg".to_string())],
        );

        Ok(())
    }

    fn get_char_size(&self) -> std::io::Result<Vector> {
        Ok(Vector {
            x: ((self.font.borrow().chars.get(&'A').unwrap().advance >> 6) as f32 * SCALE) as i32,
            y: (self.font.borrow().size as f32 * SCALE) as i32,
        })
    }

    fn end(&self) -> std::io::Result<()> {
        let mut tmp = self.win.borrow_mut();

        tmp.swap_buffers();

        Ok(())
    }
}

pub struct GlDrawer {
    pub win: RefCell<glfw::PWindow>,
    pub glfw: glfw::Glfw,
    pub events: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
    pub size: Vector,
    pub font: RefCell<GlFont>,
    pub keys: HashMap<glfw::Key, ev::Nav>,
    pub solid_program: RefCell<Option<helpers::ShaderProgram>>,
    pub cursor: RefCell<[Vector2; 4]>,
    pub cursor_targ: RefCell<[Vector2; 4]>,
    pub cursor_t: RefCell<[f32; 4]>,
    pub images: RefCell<HashMap<String, (u32, Vector)>>,
    pub mods: ev::Mods,
    pub mouse: Vector,
}

impl drawer::Drawer for GlDrawer {
    fn init(&mut self) -> std::io::Result<()> {
        self.keys.insert(glfw::Key::Up, ev::Nav::Up);
        self.keys.insert(glfw::Key::Down, ev::Nav::Down);
        self.keys.insert(glfw::Key::Left, ev::Nav::Left);
        self.keys.insert(glfw::Key::Right, ev::Nav::Right);
        self.keys.insert(glfw::Key::Escape, ev::Nav::Escape);
        self.keys.insert(glfw::Key::Enter, ev::Nav::Enter);
        self.keys.insert(glfw::Key::Backspace, ev::Nav::BackSpace);

        self.solid_program = RefCell::new(Some(
            helpers::ShaderProgram::from_vert_frag(SOLID_VERT_SHADER, SOLID_FRAG_SHADER).unwrap(),
        ));

        Ok(())
    }

    fn deinit(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn begin<'a>(
        &'a mut self,
        colors: &'a HashMap<String, highlight::Color>,
    ) -> std::io::Result<Box<dyn drawer::Handle + 'a>> {
        let result = GlHandle {
            win: &self.win,
            font: &self.font,
            program: &self.solid_program,
            cursor: &self.cursor,
            cursor_targ: &self.cursor_targ,
            cursor_t: &self.cursor_t,
            images: &self.images,
            size: Vector2 {
                x: self.size.x as f32,
                y: self.size.y as f32,
            },
            colors,
        };

        unsafe {
            if let highlight::Color::Hex { r, g, b } = result.get_color("bg".to_string()) {
                glClearColor(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
                glEnable(GL_BLEND);
                glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
                glDisable(GL_CULL_FACE);
            }
        }

        Ok(Box::new(result))
    }

    fn get_size(&self) -> std::io::Result<Vector> {
        Ok(Vector {
            x: self.size.x,
            y: self.size.y - ((self.font.borrow().size as f32) * SCALE) as i32,
        })
    }

    fn get_events(&mut self) -> Vec<ev::Event> {
        if self.win.borrow().should_close() {
            return vec![ev::Event::Quit];
        }

        self.glfw.poll_events();

        let mut result = Vec::new();

        for (_, event) in glfw::flush_messages(&self.events) {
            match event {
                glfw::WindowEvent::Size(w, h) => {
                    self.size.x = w;
                    self.size.y = h;

                    unsafe {
                        glViewport(0, 0, self.size.x, self.size.y);
                    }

                    let tmp = self.font.borrow_mut();
                    tmp.program.set_uniform_int("width\0", w);
                    tmp.program.set_uniform_int("height\0", h);

                    let prg = self.solid_program.borrow();
                    let prg = prg.as_ref().unwrap();

                    prg.set_uniform_int("width\0", w);
                    prg.set_uniform_int("height\0", h);
                }
                glfw::WindowEvent::Char(char) => {
                    let ev = ev::Event::Key(self.mods.clone(), char);
                    if !result.contains(&ev) {
                        result.push(ev)
                    }
                }
                glfw::WindowEvent::CharModifiers(char, mods) => {
                    self.mods = ev::Mods {
                        shift: mods.contains(glfw::Modifiers::Shift),
                        alt: mods.contains(glfw::Modifiers::Alt),
                        ctrl: mods.contains(glfw::Modifiers::Control),
                    };

                    let ev = ev::Event::Key(self.mods.clone(), char);
                    if !result.contains(&ev) {
                        result.push(ev)
                    }
                }
                glfw::WindowEvent::Key(k, _, glfw::Action::Press | glfw::Action::Repeat, mods)
                    if self.keys.contains_key(&k) =>
                {
                    self.mods = ev::Mods {
                        shift: mods.contains(glfw::Modifiers::Shift),
                        alt: mods.contains(glfw::Modifiers::Alt),
                        ctrl: mods.contains(glfw::Modifiers::Control),
                    };

                    result.push(ev::Event::Nav(
                        self.mods.clone(),
                        *self.keys.get(&k).unwrap(),
                    ))
                }
                glfw::WindowEvent::Key(
                    key,
                    _,
                    glfw::Action::Press | glfw::Action::Repeat,
                    mods,
                ) => {
                    self.mods = ev::Mods {
                        shift: mods.contains(glfw::Modifiers::Shift),
                        alt: mods.contains(glfw::Modifiers::Alt),
                        ctrl: mods.contains(glfw::Modifiers::Control),
                    };

                    if let Some(char) = glfw::get_key_name(Some(key), None) {
                        if let Some(mut ch) = char.chars().nth(0) {
                            if self.mods.shift {
                                if ch == '[' {
                                    ch = '{';
                                    self.mods.shift = false;
                                } else if ch == ']' {
                                    ch = '}';
                                    self.mods.shift = false;
                                } else if ch == '.' {
                                    ch = '>';
                                    self.mods.shift = false;
                                } else if ch == ',' {
                                    ch = '<';
                                    self.mods.shift = false;
                                } else if ch == ';' {
                                    ch = ':';
                                    self.mods.shift = false;
                                } else if ch == '\'' {
                                    ch = '"';
                                    self.mods.shift = false;
                                } else if ch.is_numeric() {
                                    if let Some(new_ch) =
                                        "!@#$%^&*()".chars().nth(ch as usize - '0' as usize)
                                    {
                                        ch = new_ch;
                                        self.mods.shift = false;
                                    }
                                } else if let Some(new_ch) = ch.to_uppercase().nth(0) {
                                    ch = new_ch;
                                    self.mods.shift = false;
                                };
                            }
                            let ev = ev::Event::Key(self.mods.clone(), ch);
                            if !result.contains(&ev) {
                                result.push(ev)
                            }
                        }
                    }
                }
                glfw::WindowEvent::CursorPos(x, y) => {
                    self.mouse.x = x as i32;
                    self.mouse.y = y as i32;
                }
                glfw::WindowEvent::MouseButton(btn, glfw::Action::Press, _) => {
                    result.push(ev::Event::Mouse(self.mouse, btn as i32))
                }
                _ => {}
            }
        }

        result
    }
}
