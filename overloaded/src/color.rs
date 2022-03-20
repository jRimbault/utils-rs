use std::convert::TryInto;
use std::fmt;

pub fn rgb<T>(data: T) -> Result<Rgb, Error>
where
    T: TryInto<Rgb, Error = Error>,
{
    data.try_into()
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Error {
    #[error("parsing error")]
    Parse(#[from] std::num::ParseIntError),
    #[error("value not in allowed range [0.0, 1.0] or [0, 255]")]
    NotInRange,
    #[error("invalid color string")]
    InvalidColorString,
}

/// ```
/// use std::convert::TryFrom;
/// use playground::color;
///
/// assert!(color::Hue::try_from(1.0001).is_err());
/// assert!(color::Hue::try_from(-0.0001).is_err());
/// assert!(color::Hue::try_from(-1).is_err());
/// assert!(color::Hue::try_from(256).is_err());
/// ```
#[derive(Clone, Copy, PartialOrd, PartialEq, Default)]
pub struct Hue(f64);

/// ```
/// use playground::color;
///
/// assert_eq!(
///     color::rgb((255, 0, 64)).unwrap(),
///     color::rgb("#FF0040").unwrap()
/// );
///
/// assert!(color::rgb((1.0001, 0, 1)).is_err());
/// assert!(color::rgb((-0.0001, 1, 1)).is_err());
/// assert!(color::rgb("(-0.0001, 1, 1)").is_err());
/// assert!(color::rgb("not_a_color").is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Default)]
pub struct Rgb(Hue, Hue, Hue);

impl fmt::Debug for Hue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Hue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = (self.0 * 255.0) as u8;
        fmt::Display::fmt(&c, f)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgb({}, {}, {})", self.0, self.1, self.2)
    }
}

impl fmt::UpperHex for Hue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = (self.0 * 255.0) as u8;
        write!(f, "{:0>2X}", c)
    }
}

impl fmt::UpperHex for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:X}{:X}{:X}", self.0, self.1, self.2)
    }
}

macro_rules! impl_try_from_int {
    ($integer:ty) => {
        impl TryFrom<$integer> for Hue {
            type Error = Error;
            fn try_from(i: $integer) -> Result<Self, Self::Error> {
                (0..=255)
                    .contains(&i)
                    .then(|| Self(i as f64 / 255.0))
                    .ok_or(Error::NotInRange)
            }
        }
    };
    ($integer:ty, $($integers:ty),* $(,)?) => {
        impl_try_from_int! { $integer }
        impl_try_from_int! { $($integers),* }
    };
}

macro_rules! impl_try_from_float {
    ($float:ty) => {
        impl TryFrom<$float> for Hue {
            type Error = Error;
            fn try_from(i: $float) -> Result<Self, Self::Error> {
                let h: f64 = i.into();
                (0.0..=1.0)
                    .contains(&h)
                    .then(|| Self(h))
                    .ok_or(Error::NotInRange)
            }
        }
    };
    ($float:ty, $($floats:ty),* $(,)?) => {
        impl_try_from_float! { $float }
        impl_try_from_float! { $($floats),* }
    };
}

impl_try_from_int! { u8, u16, u32, i16, i32 }
impl_try_from_float! { f32, f64 }

impl<F1, F2, F3> TryFrom<(F1, F2, F3)> for Rgb
where
    F1: TryInto<Hue, Error = Error>,
    F2: TryInto<Hue, Error = Error>,
    F3: TryInto<Hue, Error = Error>,
{
    type Error = Error;
    fn try_from((h1, h2, h3): (F1, F2, F3)) -> Result<Self, Self::Error> {
        Ok(Self(h1.try_into()?, h2.try_into()?, h3.try_into()?))
    }
}

impl TryFrom<&str> for Rgb {
    type Error = Error;
    fn try_from(mut s: &str) -> Result<Rgb, Self::Error> {
        if s.starts_with('#') {
            s = &s[1..];
        }
        if s.len() != 6 {
            return Err(Error::InvalidColorString);
        }
        Self::try_from((
            u8::from_str_radix(&s[0..2], 16)?,
            u8::from_str_radix(&s[2..4], 16)?,
            u8::from_str_radix(&s[4..6], 16)?,
        ))
    }
}
