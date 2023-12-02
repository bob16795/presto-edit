use crate::math::Vector;

#[derive(PartialEq, Debug)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(PartialEq, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Escape,
    Enter,
    BackSpace,
}

#[derive(PartialEq, Debug)]
pub enum Event {
    Key(Mods, char),
    Nav(Mods, Nav),
    Save(Option<String>),
    Mouse(Vector),
    Quit,
}
