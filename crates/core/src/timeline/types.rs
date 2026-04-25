use serde::{Deserialize, Serialize};

/// Fixed-point number in Q16.16 format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fixed(pub(crate) i32);

impl Fixed {
    pub const ONE: Fixed = Fixed(1 << 16);
    pub const ZERO: Fixed = Fixed(0);
    pub const FRAC_BITS: u32 = 16;

    #[inline]
    pub const fn from_int(n: i32) -> Self {
        Self(n << Self::FRAC_BITS)
    }

    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    #[inline]
    pub const fn to_int(self) -> i32 {
        self.0 >> Self::FRAC_BITS
    }

    #[inline]
    pub fn from_f32(f: f32) -> Self {
        Self((f * (1 << Self::FRAC_BITS) as f32) as i32)
    }

    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (1 << Self::FRAC_BITS) as f32
    }

    #[inline]
    pub fn lerp(a: Fixed, b: Fixed, t: Fixed) -> Fixed {
        let diff = b.0 as i64 - a.0 as i64;
        let scaled = (diff * t.0 as i64) >> Self::FRAC_BITS;
        Fixed(a.0 + scaled as i32)
    }
}

impl std::ops::Add for Fixed {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for Fixed {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Mul for Fixed {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let result = (self.0 as i64 * rhs.0 as i64) >> Self::FRAC_BITS;
        Self(result as i32)
    }
}

impl std::ops::Div for Fixed {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            return Self::ZERO;
        }
        let result = ((self.0 as i64) << Self::FRAC_BITS) / rhs.0 as i64;
        Self(result as i32)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Step,
}

impl Easing {
    pub fn apply(self, t: Fixed) -> Fixed {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => {
                let one_minus_t = Fixed::ONE - t;
                Fixed::ONE - (one_minus_t * one_minus_t)
            }
            Easing::EaseInOut => {
                let half = Fixed::from_raw(Fixed::ONE.0 / 2);
                if t < half {
                    let two = Fixed::from_int(2);
                    two * t * t
                } else {
                    let two = Fixed::from_int(2);
                    let one_minus_t = Fixed::ONE - t;
                    Fixed::ONE - two * one_minus_t * one_minus_t
                }
            }
            Easing::Step => {
                if t.0 >= Fixed::ONE.0 {
                    Fixed::ONE
                } else {
                    Fixed::ZERO
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    PositionX,
    PositionY,
    ZOrder,
    Scale,
    Opacity,
    Rotation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyValue {
    pub property: PropertyType,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: u32,
    pub value: i32,
    pub easing: Easing,
}

impl Keyframe {
    pub const fn new(time: u32, value: i32, easing: Easing) -> Self {
        Self {
            time,
            value,
            easing,
        }
    }
}
