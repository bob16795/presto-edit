use clap::Parser;
use core::ffi::CStr;
use dirs;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::io::stdout;
use std::path;
use std::sync::Mutex;

use glfw;
use glfw::Context;
use ogl33::*;

mod bind;
mod buffer;
mod data;
mod drawer;
mod event;
mod highlight;
mod logging;
mod lsp;
mod math;
mod script;
mod status;

use buffer::empty::*;
use buffer::file::*;
use buffer::hex::*;
use buffer::hl::*;
use buffer::logview::*;
use buffer::split::*;
use buffer::tabbed::*;
use buffer::*;
use drawer::Drawable;
use math::*;
use script::{Command, Open, SplitKind};

const DEFAULT_CONFIG: &str = include_str!("assets/default_config.pe");

pub struct Status {
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

fn prompt<'a>(
    data: &mut data::Data,
    input: String,
    default: String,
) -> std::io::Result<Option<String>> {
    data.status.prompt = Some(input);
    data.status.input = default;

    render(data)?;

    let targ_none = event::Mods {
        ctrl: false,
        alt: false,
        shift: false,
    };

    let mut done = false;

    while !done {
        for ev in data.dr.get_events() {
            match ev {
                event::Event::Nav(mods, event::Nav::Escape) if mods == targ_none => {
                    data.status.prompt = None;

                    return Ok(None);
                }
                event::Event::Nav(mods, event::Nav::Enter) if mods == targ_none => done = true,
                event::Event::Nav(mods, event::Nav::BackSpace) if mods == targ_none => {
                    _ = data.status.input.pop()
                }
                event::Event::Key(mods, c) if mods == targ_none => data.status.input.push(c),
                event::Event::Quit => done = true,
                _ => {}
            }
        }
        render(data)?;
    }

    data.status.prompt = None;

    render(data)?;

    Ok(Some(data.status.input.clone()))
}

fn render(data: &mut data::Data) -> std::io::Result<()> {
    let size = data.dr.get_size()?;
    data.bu.update(size);

    let mut handle = data.dr.begin(&data.colors)?;
    let handle = handle.as_mut();

    data.bu.draw(
        handle,
        Rect {
            x: 0,
            y: 0,
            w: size.x as i32,
            h: size.y as i32,
        },
    )?;

    let cur = data.bu.get_cursor(
        Vector {
            x: size.x as i32,
            y: size.y as i32,
        },
        handle.get_char_size()?,
    );
    handle.render_cursor(cur)?;

    data.status.path = data.bu.get_path();
    data.status.ft = format!("{:?}", data.bu.get_var(&"filetype".to_string()));

    data.status.draw(
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

fn run_command<'a, 'b>(cmd: Command, data: &mut data::Data) -> std::io::Result<()> {
    match cmd {
        Command::Unknown(s) => {
            if s != "" {
                warn!("unknown command: {}", s);
            }
        }
        Command::Incomplete(cmd) => {
            if let Some(cmd) = prompt(data, "".to_string(), cmd.to_string() + " ")? {
                let cmd = Command::parse(cmd);

                run_command(cmd, data)?;
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
            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Split pane horizontal");
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
            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Split pane vertical");
            }
        }
        Command::Split(SplitKind::Tabbed) => {
            let adds: Box<Buffer> = Box::new(TabbedBuffer {
                tabs: vec![Box::new(EmptyBuffer {}).into()],
                active: 0,
                char_size: Vector { x: 1, y: 1 },
            })
            .into();
            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Split tabbed");
            }
        }
        Command::Open(path, Open::Text) => {
            let new_path = path.clone();
            let cont = fs::read_to_string(&path);
            let adds: Box<Buffer> = Box::new(FileBuffer {
                filename: path.clone(),
                cached: false,
                data: Vec::new(),
                pos: Vector { x: 0, y: 0 },
                scroll: 0,
                mode: FileMode::Normal,
                height: 0,
                char_size: Vector { x: 0, y: 0 },
                highlight: Vec::new(),
            })
            .into();
            if let Ok(c) = cont {
                data.lsp.lock().unwrap().open_file(path, c)?;
            }
            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Opened file {}", new_path);
            }
        }
        Command::Open(path, Open::Hex) => {
            let adds: Box<Buffer> = Box::new(HexBuffer {
                filename: path.clone(),
                cached: false,
                data: Vec::new(),
                pos: Vector { x: 0, y: 0 },
                scroll: 0,
                mode: HexMode::Normal,
                height: 0,
                char_size: Vector { x: 0, y: 0 },
            })
            .into();
            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Opened hex {}", path);
            }
        }
        Command::Write(path) => {
            data.bu.as_mut().event_process(
                event::Event::Save(path),
                &mut data.lsp,
                Rect {
                    x: 0,
                    y: 0,
                    w: data.dr.get_size()?.x,
                    h: data.dr.get_size()?.y,
                },
            );
        }
        Command::Source(path) => {
            let path = if path.starts_with("~") {
                dirs::home_dir().unwrap_or("~".into()).display().to_string()
                    + path.strip_prefix("~").unwrap()
            } else {
                path
            };

            info!("source config {}", path);

            let file = fs::read_to_string(&path)?;
            for line in file.lines() {
                if let Some(cmd) = line.split("//").next() {
                    let cmd = Command::parse(cmd.trim().to_string());
                    run_command(cmd, data)?;
                }
            }
        }
        Command::Run => {
            if let Some(cmd) = prompt(data, "".to_string(), "".to_string())? {
                let cmd = Command::parse(cmd);

                run_command(cmd, data)?;
            };
        }
        Command::Close => match data.bu.close(&mut data.lsp) {
            CloseKind::Replace(r) => data.bu = r,
            CloseKind::This => {
                info!("Closed buffer {}", data.bu.get_path());
                data.bu = Box::new(EmptyBuffer {}).into();
            }
            CloseKind::Done => {}
        },
        Command::Log => {
            let adds: Box<Buffer> = Box::new(LogViewBuffer {}).into();

            if data.bu.set_focused(&adds) {
                data.bu = adds;
                info!("Opened log");
            }
        }
        Command::Highlight(None) => {
            let adds: Box<Buffer> = Box::new(HighlightBuffer {
                colors: data.colors.clone(),
            })
            .into();

            if data.bu.set_focused(&adds) {
                data.bu = adds;
            }
        }
        Command::Highlight(Some((s, None))) => {
            data.colors.remove(&s);
        }
        Command::Highlight(Some((s, Some(c)))) => {
            data.colors.insert(s, c);
        }
        Command::Bind(s, None) => {
            data.binds.remove(&s);
        }
        Command::Bind(s, Some(c)) => {
            data.binds.insert(s, *c);
        }
        Command::Set(s, None) => {
            info!("vale {} is {:?}", s, data.bu.get_var(&s));
        }
        Command::Set(s, Some(v)) => {
            if let Some(cmd) = data.auto.get(&(s.clone(), v.clone())) {
                let cmd = Command::parse(cmd.to_string());

                run_command(cmd, data)?;
            };

            if s == "lsp" {
                info!("set lsp to {}", v);
                lsp::spawn_lsp(data.lsp.clone(), v.clone())?;
                data.bu.setup_lsp(&mut data.lsp);
            }

            data.bu.set_var(s, v);
        }
        Command::Auto(var, val, cmd) => {
            data.auto.insert((var, val), cmd);
        }
        c => {
            warn!("todo: {:?}", c)
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

    logging::setup_logger();

    let mut dr: Box<dyn drawer::Drawer>;

    if args.cmd {
        dr = Box::new(drawer::cli::CliDrawer { stdout: stdout() });
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

        let font = drawer::gl::GlFont::new("font.ttf");

        dr = Box::new(drawer::gl::GlDrawer {
            glfw,
            win: std::cell::RefCell::new(win),
            events,
            size: Vector { x: 640, y: 480 },
            font: std::cell::RefCell::new(font),
            keys: HashMap::new(),
            images: std::cell::RefCell::new(HashMap::new()),
            solid_program: std::cell::RefCell::new(None),
            cursor: std::cell::RefCell::new([drawer::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
            cursor_targ: std::cell::RefCell::new([drawer::gl::Vector2 { x: 0.0, y: 0.0 }; 4]),
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
        //drawer_box = Box::new(drawer::gui::GuiDrawer {
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

    dr.init()?;

    let binds = HashMap::new();
    let colors = HashMap::new();
    let auto = HashMap::new();
    let bu: Box<Buffer> = Box::new(EmptyBuffer {}).into();
    let status = Status {
        path: "".to_string(),
        prompt: None,
        input: "".to_string(),
        ft: "".to_string(),
    };

    let mut data = data::Data {
        dr,
        bu,
        status,
        binds,
        colors,
        auto,
        lsp: Mutex::new(lsp::LSPData::new()).into(),
    };
    let mut config_dir = dirs::config_dir().unwrap_or(path::PathBuf::from("."));
    config_dir.push("prestoedit");
    let mut config_file = config_dir.clone();
    config_file.push("init");
    config_file.set_extension("pe");

    if !fs::metadata(config_dir.clone()).is_ok() {
        _ = fs::create_dir(config_dir);
    }

    if !fs::metadata(config_file.clone()).is_ok() {
        _ = fs::write(config_file.clone(), DEFAULT_CONFIG);
    }

    let cmd = Command::parse(format!("source {}", config_file.display()));
    run_command(cmd, &mut data)?;

    data.binds.insert("<S-:>".to_string(), Command::Run);

    render(&mut data)?;

    let mut done = false;

    while !done {
        for ev in data.dr.get_events() {
            match &ev {
                event::Event::Quit => done = true,
                _ => {
                    if let Some(cmd) = bind::check(&mut data.binds, &ev) {
                        run_command(cmd, &mut data)?;
                    } else {
                        data.bu.as_mut().event_process(
                            ev,
                            &mut data.lsp,
                            Rect {
                                x: 0,
                                y: 0,
                                w: data.dr.get_size()?.x,
                                h: data.dr.get_size()?.y,
                            },
                        )
                    };
                }
            }
        }
        render(&mut data)?;
        {
            let l = &mut data.lsp.lock().unwrap();
            l.update();
        }
    }

    data.dr.deinit()?;

    Ok(())
}
