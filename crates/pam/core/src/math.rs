use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};

pub type Matrix = [f32; 6];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub fn clamped(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
            a: self.a.clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn from_points(points: impl IntoIterator<Item = [f32; 2]>) -> Option<Self> {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for [x, y] in points {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        min_x.is_finite().then_some(Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

pub const IDENTITY_MATRIX: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub fn transform_to_matrix(values: &[f64]) -> Result<Matrix> {
    let matrix = match values {
        [x, y] => [1.0, 0.0, 0.0, 1.0, *x as f32, *y as f32],
        [angle, x, y] => {
            let (sin, cos) = (*angle as f32).sin_cos();
            [cos, sin, -sin, cos, *x as f32, *y as f32]
        }
        [a, b, c, d, x, y] => [
            *a as f32, *b as f32, *c as f32, *d as f32, *x as f32, *y as f32,
        ],
        values => {
            return Err(CoreError::InvalidTransform {
                actual: values.len(),
            });
        }
    };
    Ok(matrix)
}

pub fn multiply_matrix(parent: Matrix, child: Matrix) -> Matrix {
    [
        parent[0] * child[0] + parent[2] * child[1],
        parent[1] * child[0] + parent[3] * child[1],
        parent[0] * child[2] + parent[2] * child[3],
        parent[1] * child[2] + parent[3] * child[3],
        parent[0] * child[4] + parent[2] * child[5] + parent[4],
        parent[1] * child[4] + parent[3] * child[5] + parent[5],
    ]
}

pub fn multiply_color(parent: Color, child: Color) -> Color {
    Color {
        r: parent.r * child.r,
        g: parent.g * child.g,
        b: parent.b * child.b,
        a: parent.a * child.a,
    }
}

pub fn transform_point(matrix: Matrix, x: f32, y: f32) -> [f32; 2] {
    [
        matrix[0] * x + matrix[2] * y + matrix[4],
        matrix[1] * x + matrix[3] * y + matrix[5],
    ]
}
