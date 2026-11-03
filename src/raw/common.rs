//! Identifiers and small value types shared by every data file.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    )+};
}

string_id! {
    /// A game flag such as `f_DefeatedKraid`.
    FlagName,
    /// A tech such as `canWalljump`.
    TechName,
    /// A helper such as `h_canUsePowerBombs`.
    HelperName,
    /// A numeric parameter such as `n_gateGlitchLenience`.
    NumericName,
    /// An obstacle identifier, unique within a room.
    ObstacleId,
    /// A requirement such as `i_ammoRefill`, which Map Rando's fork leaves for the randomizer to define.
    InternalName,
}

/// An object that must be present but carries no data, such as `{}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// A free-text note, written upstream as either a string or a list of lines.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Note {
    Text(String),
    Lines(Vec<String>),
}

impl Note {
    /// Iterates over the note's lines, whichever form it was written in.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        let slice: &[String] = match self {
            Note::Text(text) => std::slice::from_ref(text),
            Note::Lines(lines) => lines,
        };
        slice.iter().map(String::as_str)
    }
}

/// A non-negative integer given as a constant, parameter, or arithmetic expression.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Numeric {
    Add(Vec<Numeric>),
    Mul(Vec<Numeric>),
    #[serde(untagged)]
    Literal(u32),
    #[serde(untagged)]
    Parameter(NumericName),
}

impl Numeric {
    /// Evaluates the expression, returning `None` if `lookup` lacks a parameter
    /// or the arithmetic overflows.
    pub fn eval(&self, lookup: &impl Fn(&NumericName) -> Option<i64>) -> Option<i64> {
        match self {
            Numeric::Literal(value) => Some(i64::from(*value)),
            Numeric::Parameter(name) => lookup(name),
            Numeric::Add(terms) => terms
                .iter()
                .try_fold(0i64, |sum, term| sum.checked_add(term.eval(lookup)?)),
            Numeric::Mul(factors) => factors
                .iter()
                .try_fold(1i64, |product, factor| product.checked_mul(factor.eval(lookup)?)),
        }
    }
}

/// A hexadecimal address or value such as `0x79E9F`, with its original spelling preserved.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HexValue {
    text: String,
    value: u32,
}

impl HexValue {
    pub fn value(&self) -> u32 {
        self.value
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl TryFrom<String> for HexValue {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let value = text
            .strip_prefix("0x")
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .ok_or_else(|| format!("invalid hexadecimal value `{text}`"))?;
        Ok(Self { text, value })
    }
}

impl From<HexValue> for String {
    fn from(value: HexValue) -> String {
        value.text
    }
}

/// A subpixel speed such as `$1.8`, with its original spelling preserved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExtraRunSpeed {
    text: String,
    value: f64,
}

impl ExtraRunSpeed {
    /// The speed in pixels per frame, so `$1.8` is `1.5`.
    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl TryFrom<String> for ExtraRunSpeed {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let invalid = || format!("invalid extra run speed `{text}`");
        let (whole, fraction) = text
            .strip_prefix('$')
            .and_then(|digits| digits.split_once('.'))
            .ok_or_else(invalid)?;
        let digit = |c: char| c.to_digit(16).ok_or_else(invalid);
        if whole.is_empty() || fraction.is_empty() {
            return Err(invalid());
        }
        let mut value = 0.0;
        for c in whole.chars() {
            value = value * 16.0 + f64::from(digit(c)?);
        }
        let mut scale = 1.0 / 16.0;
        for c in fraction.chars() {
            value += f64::from(digit(c)?) * scale;
            scale /= 16.0;
        }
        Ok(Self { text, value })
    }
}

impl From<ExtraRunSpeed> for String {
    fn from(value: ExtraRunSpeed) -> String {
        value.text
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_run_speed_is_hexadecimal() {
        let speed = ExtraRunSpeed::try_from("$1.8".to_owned()).unwrap();
        assert_eq!(speed.value(), 1.5);
        assert_eq!(speed.as_str(), "$1.8");
        assert!(ExtraRunSpeed::try_from("1.8".to_owned()).is_err());
        assert!(ExtraRunSpeed::try_from("$1.".to_owned()).is_err());
    }

    #[test]
    fn numeric_evaluates_nested_expressions() {
        let numeric: Numeric =
            serde_json::from_str(r#"{"mul": [{"add": ["n_x", 2]}, 3]}"#).unwrap();
        let lookup = |name: &NumericName| (name.as_str() == "n_x").then_some(4);
        assert_eq!(numeric.eval(&lookup), Some(18));
        assert_eq!(numeric.eval(&|_: &NumericName| None), None);
    }
}
