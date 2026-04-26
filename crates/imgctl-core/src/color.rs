use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRgba(pub [u8; 4]);

impl ColorRgba {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub fn r(&self) -> u8 { self.0[0] }
    pub fn g(&self) -> u8 { self.0[1] }
    pub fn b(&self) -> u8 { self.0[2] }
    pub fn a(&self) -> u8 { self.0[3] }

    pub fn parse(input: &str) -> Result<Self, Error> {
        let s = input.strip_prefix('#').unwrap_or(input);
        let bytes = match s.len() {
            3 => [
                expand_nibble(&s[0..1])?,
                expand_nibble(&s[1..2])?,
                expand_nibble(&s[2..3])?,
                255,
            ],
            6 => [
                parse_byte(&s[0..2])?,
                parse_byte(&s[2..4])?,
                parse_byte(&s[4..6])?,
                255,
            ],
            8 => [
                parse_byte(&s[0..2])?,
                parse_byte(&s[2..4])?,
                parse_byte(&s[4..6])?,
                parse_byte(&s[6..8])?,
            ],
            _ => return Err(Error::InvalidArgument(format!("invalid color: {input}"))),
        };
        Ok(Self(bytes))
    }
}

fn expand_nibble(s: &str) -> Result<u8, Error> {
    let n = u8::from_str_radix(s, 16)
        .map_err(|_| Error::InvalidArgument(format!("invalid hex nibble: {s}")))?;
    Ok((n << 4) | n)
}

fn parse_byte(s: &str) -> Result<u8, Error> {
    u8::from_str_radix(s, 16)
        .map_err(|_| Error::InvalidArgument(format!("invalid hex byte: {s}")))
}

impl FromStr for ColorRgba {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<ColorRgba> for [u8; 4] {
    fn from(c: ColorRgba) -> Self {
        c.0
    }
}

impl Serialize for ColorRgba {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let s = format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        );
        ser.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for ColorRgba {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_3_digits() {
        assert_eq!(ColorRgba::parse("#fff").unwrap().0, [255, 255, 255, 255]);
        assert_eq!(ColorRgba::parse("#f00").unwrap().0, [255, 0, 0, 255]);
        assert_eq!(ColorRgba::parse("#abc").unwrap().0, [0xAA, 0xBB, 0xCC, 255]);
    }

    #[test]
    fn parse_6_digits() {
        assert_eq!(ColorRgba::parse("#FF0080").unwrap().0, [255, 0, 128, 255]);
        assert_eq!(ColorRgba::parse("#000000").unwrap().0, [0, 0, 0, 255]);
    }

    #[test]
    fn parse_8_digits() {
        assert_eq!(ColorRgba::parse("#FF008040").unwrap().0, [255, 0, 128, 64]);
    }

    #[test]
    fn parse_without_hash() {
        assert_eq!(ColorRgba::parse("FFFFFF").unwrap().0, [255, 255, 255, 255]);
        assert_eq!(ColorRgba::parse("123456").unwrap().0, [0x12, 0x34, 0x56, 255]);
    }

    #[test]
    fn parse_invalid() {
        assert!(ColorRgba::parse("xyz").is_err());
        assert!(ColorRgba::parse("#12").is_err());
        assert!(ColorRgba::parse("#1234567").is_err());
        assert!(ColorRgba::parse("#GGGGGG").is_err());
        assert!(ColorRgba::parse("").is_err());
    }

    #[test]
    fn from_str_works() {
        let c: ColorRgba = "#FF0000".parse().unwrap();
        assert_eq!(c.0, [255, 0, 0, 255]);
    }

    #[test]
    fn serde_roundtrip() {
        let c = ColorRgba::rgba(10, 20, 30, 40);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#0A141E28\"");
        let back: ColorRgba = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }
}
