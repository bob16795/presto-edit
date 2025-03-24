use crate::highlight::{parse_color, Color};

#[derive(Debug, Clone)]
pub enum SplitKind {
    Horizontal,
    Vertical,
    Tabbed,
}

#[derive(Debug, Clone)]
pub enum Open {
    Text,
    Hex,
}

impl SplitKind {
    pub fn parse(cmd: String) -> Self {
        match cmd.to_lowercase().as_str() {
            "horizontal" | "h" => SplitKind::Horizontal,
            "vertical" | "v" => SplitKind::Vertical,
            _ => SplitKind::Tabbed,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Unknown(String),
    Incomplete(String),
    Split(SplitKind),
    Open(String, Open),
    Write(Option<String>),
    Source(String),
    Bind(String, Option<Box<Command>>),
    Highlight(Option<(String, Option<Color>)>),
    Set(String, Option<String>),
    Auto(String, String, String),
    Log,
    Run,
    Close,
    Exit,
}

impl Command {
    pub fn parse(cmd: String) -> Self {
        let mut split = cmd.split_whitespace();
        match split.next() {
            Some("source" | "src") => match split.next() {
                Some(s) => Command::Source(s.to_string()),
                None => Command::Incomplete(cmd),
            },
            Some("split" | "s") => match split.next() {
                Some(s) => Command::Split(SplitKind::parse(s.to_string())),
                None => Command::Incomplete(cmd),
            },
            Some("openhex" | "oh") => match split.next() {
                Some(s) => Command::Open(s.to_string(), Open::Hex),
                None => Command::Incomplete(cmd),
            },
            Some("open" | "o") => match split.next() {
                Some(s) => Command::Open(s.to_string(), Open::Text),
                None => Command::Incomplete(cmd),
            },
            Some("write" | "w") => match split.next() {
                Some(s) => Command::Write(Some(s.to_string())),
                None => Command::Write(None),
            },
            Some("bind" | "b") => match (
                split.next(),
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), c) if c.len() == 0 => Command::Bind(s.to_string(), None),
                (Some(s), c) => {
                    let cmd = Self::parse(c.to_string());
                    Command::Bind(s.to_string(), Some(Box::new(cmd)))
                }
                _ => Command::Incomplete(cmd),
            },
            Some("auto" | "a") => match (
                split.next(),
                split.next(),
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), Some(t), c) => Command::Auto(s.to_string(), t.to_string(), c),
                _ => Command::Incomplete(cmd),
            },
            Some("set") => match (
                split.next(),
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), c) if c.len() == 0 => Command::Set(s.to_string(), None),
                (Some(s), c) => Command::Set(s.to_string(), Some(c)),
                _ => Command::Incomplete(cmd),
            },
            Some("quit" | "q") => Command::Close,
            Some("exit" | "e") => Command::Exit,
            Some("log") => Command::Log,
            Some("highlight" | "hi") => match (
                split.next(),
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), c) if c.len() == 0 => Command::Highlight(Some((s.to_string(), None))),
                (Some(s), c) => {
                    let color = parse_color(c.to_string()).unwrap();
                    Command::Highlight(Some((s.to_string(), Some(color))))
                }
                _ => Command::Highlight(None),
            },
            _ => Command::Unknown(cmd),
        }
    }
}
