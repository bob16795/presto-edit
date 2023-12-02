use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
pub enum Color {
    Invalid,
    Base16(u8),
    Hex { r: u8, g: u8, b: u8 },
    Link(String),
}

pub fn get_color<'a>(map: &HashMap<String, Color>, c: Color) -> Option<Color> {
    match c {
        Color::Link(s) => match map.get(&s) {
            Some(c) => get_color(map, c.clone()),
            None => None,
        },
        _ => Some(c),
    }
}

pub fn parse_color<'a>(color: String) -> Option<Color> {
    if color.chars().nth(0) == Some('%') {
        Some(Color::Link(color[1..].to_string()))
    } else if color.chars().nth(0) == Some('#') {
        if color.len() - 1 == 6 {
            let c = i64::from_str_radix(&color[1..], 16).unwrap();
            Some(Color::Hex {
                r: ((c & 0xFF0000) >> 16) as u8,
                g: ((c & 0x00FF00) >> 8) as u8,
                b: ((c & 0x0000FF) >> 0) as u8,
            })
        } else {
            Some(Color::Invalid)
        }
    } else {
        Some(Color::Invalid)
    }
}
