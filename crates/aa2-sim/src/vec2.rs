use std::ops::{Add, Sub};

/// 2D vector for game-world positions and directions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self { x: self.x + other.x, y: self.y + other.y }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self { x: self.x - other.x, y: self.y - other.y }
    }
}

impl Vec2 {
    /// Create a new Vec2.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Zero vector.
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Euclidean distance to another point.
    pub fn distance(self, other: Self) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Length of the vector.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Normalize to unit length. Returns zero vector if length is ~0.
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-6 {
            Self::zero()
        } else {
            Self { x: self.x / len, y: self.y / len }
        }
    }

    /// Angle in radians from positive X axis (atan2).
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Scale vector by scalar.
    pub fn scale(self, s: f32) -> Self {
        Self { x: self.x * s, y: self.y * s }
    }
}
