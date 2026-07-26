use image::Rgba;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba32 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba32 {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_pixel(p: Rgba<u8>) -> Self {
        Self {
            r: p[0],
            g: p[1],
            b: p[2],
            a: p[3],
        }
    }

    pub fn to_pixel(self) -> Rgba<u8> {
        Rgba([self.r, self.g, self.b, self.a])
    }
}

impl Default for Rgba32 {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

// Mimic C# ColorRGBA for arithmetic
#[derive(Clone, Copy, Debug)]
pub struct ColorRGBA {
    pub r: i32,
    pub g: i32,
    pub b: i32,
    pub a: i32,
}

impl ColorRGBA {
    pub fn new(r: i32, g: i32, b: i32, a: i32) -> Self {
        Self { r, g, b, a }
    }
}

impl std::ops::Add for ColorRGBA {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            r: self.r + other.r,
            g: self.g + other.g,
            b: self.b + other.b,
            a: self.a + other.a,
        }
    }
}

impl std::ops::Sub for ColorRGBA {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            r: self.r - other.r,
            g: self.g - other.g,
            b: self.b - other.b,
            a: self.a - other.a,
        }
    }
}

impl std::ops::Mul<i32> for ColorRGBA {
    type Output = Self;
    fn mul(self, scalar: i32) -> Self {
        Self {
            r: self.r * scalar,
            g: self.g * scalar,
            b: self.b * scalar,
            a: self.a * scalar,
        }
    }
}

impl std::ops::Rem<ColorRGBA> for ColorRGBA {
    type Output = i32; // Dot product? In code: color.r * x.r + ...
    fn rem(self, other: Self) -> i32 {
        self.r * other.r + self.g * other.g + self.b * other.b + self.a * other.a
    }
}

// Mimic C# ColorRGB for arithmetic
#[derive(Clone, Copy, Debug)]
pub struct ColorRGB {
    pub r: i32,
    pub g: i32,
    pub b: i32,
}

impl ColorRGB {
    pub fn new(r: i32, g: i32, b: i32) -> Self {
        Self { r, g, b }
    }
}

impl std::ops::Add for ColorRGB {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            r: self.r + other.r,
            g: self.g + other.g,
            b: self.b + other.b,
        }
    }
}

impl std::ops::Sub for ColorRGB {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            r: self.r - other.r,
            g: self.g - other.g,
            b: self.b - other.b,
        }
    }
}

impl std::ops::Mul<i32> for ColorRGB {
    type Output = Self;
    fn mul(self, scalar: i32) -> Self {
        Self {
            r: self.r * scalar,
            g: self.g * scalar,
            b: self.b * scalar,
        }
    }
}

impl std::ops::Rem<ColorRGB> for ColorRGB {
    type Output = i32;
    fn rem(self, other: Self) -> i32 {
        self.r * other.r + self.g * other.g + self.b * other.b
    }
}
