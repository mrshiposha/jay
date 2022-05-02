use std::cell::{Cell, RefCell};

#[derive(Copy, Clone, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const GREY: Self = Self {
        r: 0.8,
        g: 0.8,
        b: 0.8,
        a: 1.0,
    };

    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
}

fn to_f32(c: u8) -> f32 {
    c as f32 / 255f32
}

fn to_u8(c: f32) -> u8 {
    (c * 255f32) as u8
}

impl Color {
    pub fn from_rgba_straight(r: u8, g: u8, b: u8, a: u8) -> Self {
        let alpha = to_f32(a);
        Self {
            r: to_f32(r) * alpha,
            g: to_f32(g) * alpha,
            b: to_f32(b) * alpha,
            a: alpha,
        }
    }

    #[cfg_attr(not(feature = "it"), allow(dead_code))]
    pub fn to_rgba_premultiplied(self) -> [u8; 4] {
        [to_u8(self.r), to_u8(self.g), to_u8(self.b), to_u8(self.a)]
    }
}

impl From<jay_config::theme::Color> for Color {
    fn from(f: jay_config::theme::Color) -> Self {
        Self {
            r: to_f32(f.r),
            g: to_f32(f.g),
            b: to_f32(f.b),
            a: to_f32(f.a),
        }
    }
}

pub struct Theme {
    pub background_color: Cell<Color>,
    pub title_color: Cell<Color>,
    pub active_title_color: Cell<Color>,
    pub underline_color: Cell<Color>,
    pub border_color: Cell<Color>,
    pub last_active_color: Cell<Color>,
    pub title_height: Cell<i32>,
    pub border_width: Cell<i32>,
    pub font: RefCell<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_color: Cell::new(Color::from_rgba_straight(0x00, 0x10, 0x19, 255)),
            last_active_color: Cell::new(Color::from_rgba_straight(0x5f, 0x67, 0x6a, 255)),
            title_color: Cell::new(Color::from_rgba_straight(0x22, 0x22, 0x22, 255)),
            active_title_color: Cell::new(Color::from_rgba_straight(0x28, 0x55, 0x77, 255)),
            underline_color: Cell::new(Color::from_rgba_straight(0x33, 0x33, 0x33, 255)),
            border_color: Cell::new(Color::from_rgba_straight(0x3f, 0x47, 0x4a, 255)),
            title_height: Cell::new(17),
            border_width: Cell::new(4),
            font: RefCell::new("monospace 8".to_string()),
        }
    }
}
