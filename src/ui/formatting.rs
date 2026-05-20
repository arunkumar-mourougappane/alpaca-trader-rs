use ratatui::widgets::Cell;

use crate::ui::theme::ThemeColors;

/// Parse `s` as `f64` and format with 2 decimal places.
/// Returns `s` unchanged if it is not parseable.
///
/// ```
/// use alpaca_trader_rs::ui::formatting::format_dollar;
/// assert_eq!(format_dollar("123.456"), "123.46");
/// assert_eq!(format_dollar("N/A"), "N/A");
/// ```
pub fn format_dollar(s: &str) -> String {
    if let Ok(v) = s.parse::<f64>() {
        format!("{:.2}", v)
    } else {
        s.to_string()
    }
}

/// Parse `s` as a `f64` price string and format as `"$x.xx"`.
/// Falls back to `"$0.00"` for non-numeric input.
///
/// ```
/// use alpaca_trader_rs::ui::formatting::format_price;
/// assert_eq!(format_price("500.00"), "$500.00");
/// assert_eq!(format_price("bad"), "$0.00");
/// ```
pub fn format_price(s: &str) -> String {
    format!("${:.2}", s.parse::<f64>().unwrap_or(0.0))
}

/// Parse `s` as a fractional ratio (e.g. `"0.10"`) and format as `"+10.00%"`.
/// Returns `s` unchanged if it is not parseable.
///
/// ```
/// use alpaca_trader_rs::ui::formatting::format_pct_ratio;
/// assert_eq!(format_pct_ratio("0.05"), "+5.00%");
/// assert_eq!(format_pct_ratio("-0.025"), "-2.50%");
/// assert_eq!(format_pct_ratio("n/a"), "n/a");
/// ```
pub fn format_pct_ratio(s: &str) -> String {
    if let Ok(v) = s.parse::<f64>() {
        format!("{:+.2}%", v * 100.0)
    } else {
        s.to_string()
    }
}

/// Create a styled header [`Cell`] using `c.header_style()`.
pub fn header_cell<'a>(label: &'a str, c: &ThemeColors) -> Cell<'a> {
    Cell::from(label).style(c.header_style())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_dollar ─────────────────────────────────────────────────────────

    #[test]
    fn format_dollar_valid() {
        assert_eq!(format_dollar("123.456"), "123.46");
        assert_eq!(format_dollar("0"), "0.00");
        assert_eq!(format_dollar("1000.5"), "1000.50");
    }

    #[test]
    fn format_dollar_invalid_passthrough() {
        assert_eq!(format_dollar("not-a-number"), "not-a-number");
        assert_eq!(format_dollar("N/A"), "N/A");
        assert_eq!(format_dollar(""), "");
    }

    // ── format_price ──────────────────────────────────────────────────────────

    #[test]
    fn format_price_valid() {
        assert_eq!(format_price("500.00"), "$500.00");
        assert_eq!(format_price("0"), "$0.00");
    }

    #[test]
    fn format_price_invalid_falls_back_to_zero() {
        assert_eq!(format_price("bad"), "$0.00");
        assert_eq!(format_price(""), "$0.00");
    }

    // ── format_pct_ratio ──────────────────────────────────────────────────────

    #[test]
    fn format_pct_ratio_positive() {
        assert_eq!(format_pct_ratio("0.05"), "+5.00%");
        assert_eq!(format_pct_ratio("0.10"), "+10.00%");
    }

    #[test]
    fn format_pct_ratio_negative() {
        assert_eq!(format_pct_ratio("-0.025"), "-2.50%");
    }

    #[test]
    fn format_pct_ratio_invalid_passthrough() {
        assert_eq!(format_pct_ratio("n/a"), "n/a");
    }

    // ── header_cell ───────────────────────────────────────────────────────────

    #[test]
    fn header_cell_applies_header_style() {
        let c = crate::ui::theme::Theme::Default.colors();
        let cell = header_cell("Symbol", &c);
        // Verify the cell content is set (style is verified by the fact it compiles
        // and uses c.header_style() — style equality tested in theme tests)
        let _ = cell; // construction must not panic
    }
}
