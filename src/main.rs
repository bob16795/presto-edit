use clap::Parser;
use core::ffi::CStr;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::io::stdout;

use glfw;
use glfw::Context;
use ogl33::*;

mod bind;
mod buffer;
mod buffers {
    pub mod empty;
    pub mod file;
    pub mod hl;
    pub mod split;
    pub mod tabbed;
    pub mod tree;
}
mod drawer;
mod drawers {
    pub mod cli;
    pub mod gl;
    pub mod gui;
    pub mod helpers;
}
mod event;
mod highlight;
mod lsp;
mod math;
mod script;
mod status;

use crate::buffer::*;
use crate::buffers::empty::*;
use crate::buffers::file::*;
use crate::buffers::hl::*;
use crate::buffers::split::*;
use crate::buffers::tabbed::*;
use crate::drawer::Drawable;
use crate::math::*;
use crate::script::{Command, SplitKind};

struct Status {
    path: String,
    prompt: Option<String>,
    input: String,
    ft: String,
}

impl drawer::Drawable for Status {
    fn draw(&self, handle: &mut dyn drawer::Handle, coords: Rect) -> std::io::Result<()> {
        let left = match &self.prompt {
            Some(p) => format!("{}:{}", p, self.input),
            None => format!("{}", self.path),
        };

        handle.render_status(
            status::Status {
                left,
                center: "".to_string(),
                right: self.ft.clone() + &" | PrestoEdit".to_string(),
            },
            coords,
        )?;

        Ok(())
    }
}

impl Status {
    fn prompt<'a>(
        &mut self,
        input: String,
        drawer: &mut dyn drawer::Drawer,
        bu: &mut Box<Buffer>,
        default: String,
        colors: &'a HashMap<String, highlight::Color>,
    ) -> std::io::Result<Option<String>> {
        self.prompt = Some(input);
        self.input = default;

        render(drawer, bu.as_mut(), self, colors)?;

        let targ_none = event::Mods {
            ctrl: false,
            alt: false,
            shift: false,
        };

        let mut done = false;

        while !done {
            for ev in (drawer as &mut dyn drawer::Drawer).get_events() {
                match ev {
                    event::Event::Nav(mods, event::Nav::Escape) if mods == targ_none => {
                        self.prompt = None;

                        return Ok(None);
                    }
                    event::Event::Nav(mods, event::Nav::Enter) if mods == targ_none => done = true,
                    event::Event::Nav(mods, event::Nav::BackSpace) if mods == targ_none => {
                        _ = self.input.pop()
                    }
                    event::Event::Key(mods, c) if mods == targ_none => self.input.push(c),
                    event::Event::Quit => done = true,
                    _ => {}
                }
            }
            render(drawer, bu.as_mut(), self, colors)?;
        }

        self.prompt = None;

        render(drawer, bu.as_mut(), self, colors)?;

        Ok(Some(self.input.clone()))
    }
}

fn render<'a, 'c, 'd, 'e>(
    drawer: &mut dyn drawer::Drawer,
    bu: &'a mut Buffer,
    status: &'c mut Status,
    colors: &'e HashMap<String, highlight::Color>,
) -> std::io::Result<()> {
    let size = drawer.get_size()?;
    bu.update(size);

    let mut handle = drawer.begin(colors)?;
    let handle = handle.as_mut();

    bu.draw(
        handle,
        Rect {
            x: 0,
            y: 0,
            w: size.x as i32,
            h: size.y as i32,
        },
    )?;

    let cur = bu.get_cursor(
        Vector {
            x: size.x as i32,
            y: size.y as i32,
        },
        handle.get_char_size()?,
    );
    handle.render_cursor(cur)?;

    status.path = bu.get_path();
    status.ft = format!("{:?}", bu.get_var(&"filetype".to_string()));

    status.draw(
        handle,
        Rect {
            x: 0,
            y: size.y - 1,
            w: size.x as i32,
            h: 1,
        },
    )?;

    handle.end()?;

    Ok(())
}

fn run_command<'a, 'b>(
    cmd: Command,
    dr: &mut dyn drawer::Drawer,
    bu: &mut Box<Buffer>,
    status: &mut Status,
    binds: &mut HashMap<String, Command>,
    colors: &mut HashMap<String, highlight::Color>,
    lsp: &mut lsp::LSP,
) -> std::io::Result<()> {
    match cmd {
        Command::Unknown(_) => {}
        Command::Incomplete(cmd) => {
            if let Some(cmd) =
                status.prompt("".to_string(), dr, bu, cmd.to_string() + " ", colors)?
            {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            };
        }
        Command::Split(SplitKind::Horizontal) => {
            let adds: Box<Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}).into(),
                b: Box::new(EmptyBuffer {}).into(),
                split_dir: SplitDir::Horizontal,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Vertical) => {
            let adds: Box<Buffer> = Box::new(SplitBuffer {
                a: Box::new(EmptyBuffer {}).into(),
                b: Box::new(EmptyBuffer {}).into(),
                split_dir: SplitDir::Vertical,
                a_active: false,
                split: Measurement::Percent(0.5),
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Split(SplitKind::Tabbed) => {
            let adds: Box<Buffer> = Box::new(TabbedBuffer {
                tabs: vec![Box::new(EmptyBuffer {}).into()],
                active: 0,
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Open(path) => {
            let cont = read_to_string(&path);
            let adds: Box<Buffer> = Box::new(FileBuffer {
                filename: path.clone(),
                cached: false,
                data: Vec::new(),
                pos: Vector { x: 0, y: 0 },
                scroll: 0,
                mode: FileMode::Normal,
                height: 0,
                char_size: Vector { x: 0, y: 0 },
            })
            .into();
            if let Ok(c) = cont {
                lsp.open_file(path, c)?;
            }
            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Write(path) => {
            bu.as_mut().event_process(
                event::Event::Save(path),
                lsp,
                Rect {
                    x: 0,
                    y: 0,
                    w: dr.get_size()?.x,
                    h: dr.get_size()?.y,
                },
            );
        }
        Command::Source(path) => {
            let file = read_to_string(&path)?;
            for line in file.lines() {
                let cmd = Command::parse(line.to_string());

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            }
        }
        Command::Run => {
            if let Some(cmd) = status.prompt("".to_string(), dr, bu, "".to_string(), &colors)? {
                let cmd = Command::parse(cmd);

                run_command(cmd, dr, bu, status, binds, colors, lsp)?;
            };
        }
        Command::Close => match bu.close(lsp) {
            CloseKind::Replace(r) => *bu = r,
            CloseKind::This => *bu = Box::new(EmptyBuffer {}).into(),
            CloseKind::Done => {}
        },
        Command::Highlight(None) => {
            let adds: Box<Buffer> = Box::new(HighlightBuffer {
                colors: colors.clone(),
            })
            .into();

            if bu.set_focused(&adds) {
                *bu = adds;
            }
        }
        Command::Highlight(Some((s, None))) => {
            colors.remove(&s);
        }
        Command::Highlight(Some((s, Some(c)))) => {
            colors.insert(s, c);
        }
        Command::Bind(s, None) => {
            binds.remove(&s);
        }
        Command::Bind(s, Some(c)) => {
            binds.insert(s, *c);
        }
        Command::Set(s, None) => {
            println!("{:?}", bu.get_var(&s));
        }
        Command::Set(s, Some(v)) => {
            bu.set_var(s, v);
        }
        c => {
            println!("todo{:?}", c)
        }
    }
    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "false")]
    cmd: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    let mut drawer_box: Box<dyn drawer::Drawer>;

    if args.cmd {
        drawer_box = Box::new(drawers::cli::CliDrawer { stdout: stdout() });
    } else {
        let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();
        glfw.window_hint(glfw::WindowHint::Samples(Some(4)));

        let (mut win, events) = glfw
            .create_window(1366, 768, "PrestoEdit", glfw::WindowMode::Windowed)
            .unwrap();

        unsafe {
            load_gl_with(|f_name| win.get_proc_address(CStr::from_ptr(f_name).to_str().unwrap()))
        }
        win.make_current();
        win.set_all_polling(true);

        glfw.set_swap_interval(glfw::SwapInterval::Adaptive);

        let font = drawers::gl::GlFont::new("font.ttf");

        drawer_box = Box::new(drawers::gl::GlDrawer {
            glfw,
            win: std::cell::RefCell::new(win),
            events,
            size: Vector { x: 640, y: 480 },
            font: std::cell::RefCell::new(font),
            keys: HashMap::new(),
            images: std::cell::RefCell::new(HashMap::new()),
            solid_program: std::cell::RefCell::new(None),
            cursor: std::cell::RefCell::new([drawers::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
            cursor_targ: std::cell::RefCell::new([drawers::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
            cursor_t: std::cell::RefCell::new([0.0; 4]),
            mods: event::Mods {
                shift: false,
                alt: false,
                ctrl: false,
            },
            mouse: Vector { x: 0, y: 0 },
        });

        //let (mut rl, thread) = raylib::init()
        //    .msaa_4x()
        //    .resizable()
        //    .title("PrestoEdit")
        //    .build();
        //rl.set_target_fps(60);
        //drawer_box = Box::new(drawers::gui::GuiDrawer {
        //    rl,
        //    thread,
        //    font: None,
        //    cursor: std::cell::RefCell::new([
        //        raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
        //    ]),
        //    cursor_targ: std::cell::RefCell::new([
        //        raylib::prelude::Vector2 { x: 0.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 1.0 },
        //        raylib::prelude::Vector2 { x: 1.0, y: 0.0 },
        //        raylib::prelude::Vector2 { x: 0.0, y: 1.0 },
        //    ]),
        //    cursor_t: std::cell::RefCell::new([0.0; 4]),
        //});
    };

    let drawer: &mut dyn drawer::Drawer = drawer_box.as_mut();
    drawer.init()?;

    let mut binds = HashMap::new();
    let mut colors = HashMap::new();
    let mut bu: Box<Buffer> = Box::new(EmptyBuffer {}).into();
    let mut status = Status {
        path: "".to_string(),
        prompt: None,
        input: "".to_string(),
        ft: "".to_string(),
    };

    let mut lsp = lsp::LSP::new();
    lsp.init()?;

    let cmd = Command::parse("source /home/john/.config/prestoedit/init.pe".to_string());
    run_command(
        cmd,
        drawer,
        &mut bu,
        &mut status,
        &mut binds,
        &mut colors,
        &mut lsp,
    )?;

    binds.insert("<S-:>".to_string(), Command::Run);

    render(drawer, bu.as_mut(), &mut status, &colors)?;

    let mut done = false;

    while !done {
        for ev in drawer.get_events() {
            match &ev {
                event::Event::Quit => done = true,
                _ => {
                    if let Some(cmd) = bind::check(&mut binds, &ev) {
                        run_command(
                            cmd,
                            drawer,
                            &mut bu,
                            &mut status,
                            &mut binds,
                            &mut colors,
                            &mut lsp,
                        )?;
                    } else {
                        bu.as_mut().event_process(
                            ev,
                            &mut lsp,
                            Rect {
                                x: 0,
                                y: 0,
                                w: drawer.get_size()?.x,
                                h: drawer.get_size()?.y,
                            },
                        )
                    };
                }
            }
        }
        render(drawer, bu.as_mut(), &mut status, &colors)?;
    }

    drawer.deinit()?;

    Ok(())
}
