use crate::event::{Event, Nav};
use crate::script::Command;
use std::collections::HashMap;

pub fn check<'a>(map: &mut HashMap<String, Command>, ev: &Event) -> Option<Command> {
    match ev {
        Event::Key(mods, char) => {
            let mut name = "<".to_string();
            if mods.ctrl {
                name.push_str("C-");
            }
            if mods.alt {
                name.push_str("A-");
            }
            if mods.shift {
                name.push_str("S-");
            }
            name.push((*char).to_ascii_uppercase());
            name.push_str(">");

            match map.get(&name) {
                None => None,
                Some(&ref v) => Some(v.clone()),
            }
        }
        Event::Nav(mods, nav) => {
            let mut name = "<".to_string();
            if mods.ctrl {
                name.push_str("C-");
            }
            if mods.alt {
                name.push_str("A-");
            }
            if mods.shift {
                name.push_str("S-");
            }
            name.push_str(match *nav {
                Nav::Up => "UP",
                Nav::Down => "DOWN",
                Nav::Left => "LEFT",
                Nav::Right => "RIGHT",
                Nav::Escape => "ESC",
                Nav::Enter => "ENTER",
                Nav::BackSpace => "BS",
            });
            name.push_str(">");

            match map.get(&name) {
                None => None,
                Some(&ref v) => Some(v.clone()),
            }
        }
        _ => None,
    }
}
