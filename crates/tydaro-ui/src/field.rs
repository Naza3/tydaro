//! Input appearance and bounded-number parsing; entities remain window-owned.
use gpui_kit::{
    component::input::{Input, InputState},
    prelude::*,
    *,
};

use crate::tokens::Palette;

pub fn input(state: &Entity<InputState>, id: impl Into<ElementId>) -> Input {
    Input::new(state).id(id).w_full()
}

pub fn validated_input(
    state: &Entity<InputState>,
    id: impl Into<ElementId>,
    invalid: bool,
    palette: Palette,
) -> Input {
    input(state, id)
        .border_color(rgb(if invalid {
            palette.error
        } else {
            palette.border
        }))
        .focus_bordered(!invalid)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    Invalid,
    OutsideRange,
    Precision,
}

/// UI validation of an explicitly supplied range; no domain-specific defaults.
#[derive(Clone, Copy, Debug)]
pub struct NumberRange {
    pub min: f64,
    pub max: f64,
    pub decimals: u8,
}

impl NumberRange {
    pub fn parse(self, text: &str) -> Result<f64, NumberError> {
        let value = text
            .trim()
            .parse::<f64>()
            .map_err(|_| NumberError::Invalid)?;
        if !value.is_finite()
            || !self.min.is_finite()
            || !self.max.is_finite()
            || self.min > self.max
            || !(self.min..=self.max).contains(&value)
        {
            return Err(NumberError::OutsideRange);
        }
        let scale = 10_f64.powi(i32::from(self.decimals));
        let scaled = value * scale;
        if !scaled.is_finite() || (scaled - scaled.round()).abs() > 0.000_001 {
            return Err(NumberError::Precision);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{NumberError, NumberRange};

    #[test]
    fn number_range_rejects_non_finite_values_and_out_of_range_numbers() {
        let range = NumberRange {
            min: 0.5,
            max: 10.,
            decimals: 1,
        };
        for text in ["NaN", "inf", "-inf", "0.49", "10.01"] {
            assert_eq!(range.parse(text), Err(NumberError::OutsideRange));
        }
        assert_eq!(range.parse(""), Err(NumberError::Invalid));
        assert_eq!(range.parse("1.25"), Err(NumberError::Precision));
        assert_eq!(range.parse(" 0.5 "), Ok(0.5));
        assert_eq!(range.parse("10.0"), Ok(10.));
    }

    #[test]
    fn number_range_preserves_decimal_steps_and_rejects_invalid_configuration() {
        let range = NumberRange {
            min: -1.,
            max: 1.,
            decimals: 2,
        };
        assert_eq!(range.parse("0.29"), Ok(0.29));
        assert_eq!(range.parse("-0.37"), Ok(-0.37));
        assert_eq!(range.parse("0.291"), Err(NumberError::Precision));
        assert_eq!(
            NumberRange {
                min: 2.,
                max: 1.,
                decimals: 0
            }
            .parse("1"),
            Err(NumberError::OutsideRange)
        );
        assert_eq!(
            NumberRange {
                min: 0.,
                max: 1e300,
                decimals: 200
            }
            .parse("1e300"),
            Err(NumberError::Precision)
        );
    }
}
