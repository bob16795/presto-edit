use crate::highlight::{parse_color, Color};

#[derive(Debug, Clone)]
pub enum SplitKind {
    Horizontal,
    Vertical,
    Tabbed,
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
    Open(String),
    Write(Option<String>),
    Source(String),
    Bind(String, Option<Box<Command>>),
    Highlight(String, Option<Color>),
    Set(String, Option<String>),
    Auto(String, String),
    Run,
    Close,
    Exit,
}

impl Command {
    pub fn parse(cmd: String) -> Self {
        let mut split = cmd.split_whitespace();
        match split.next() {
            Some("source") => match split.next() {
                Some(s) => Command::Source(s.to_string()),
                None => Command::Incomplete(cmd),
            },
            Some("split" | "s") => match split.next() {
                Some(s) => Command::Split(SplitKind::parse(s.to_string())),
                None => Command::Incomplete(cmd),
            },
            Some("open" | "o") => match split.next() {
                Some(s) => Command::Open(s.to_string()),
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
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), c) if c.len() == 0 => Command::Set(s.to_string(), None),
                (Some(s), c) => Command::Set(s.to_string(), Some(c)),
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
            Some("highlight" | "hi") => match (
                split.next(),
                split.map(|s| &*s).collect::<Vec<&str>>().join(" "),
            ) {
                (Some(s), c) if c.len() == 0 => Command::Highlight(s.to_string(), None),
                (Some(s), c) => {
                    let color = parse_color(c.to_string()).unwrap();
                    Command::Highlight(s.to_string(), Some(color))
                }
                _ => Command::Incomplete(cmd),
            },
            _ => Command::Unknown(cmd),
        }
    }
}
