#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Vector {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Copy, Clone)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum Measurement {
    Percent(f32),
    Chars(usize),
    NegChars(usize),
    Pixels(usize),
    NegPixels(usize),
}

impl Measurement {
    pub fn get_value(&self, max: usize, char_size: usize) -> usize {
        match &self {
            Self::Percent(pc) => (max as f32 * pc) as usize,
            Self::Chars(val) => (*val * char_size).min(max),
            Self::NegChars(val) => max - (*val * char_size).min(max),
            Self::Pixels(val) => (*val).min(max),
            Self::NegPixels(val) => max - val.min(&max),
        }
    }
}
