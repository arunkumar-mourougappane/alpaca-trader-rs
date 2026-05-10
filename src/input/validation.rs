use crate::app::OrderEntryState;

/// Validates an [`OrderEntryState`] before a submit command is dispatched.
///
/// Returns `None` when the state is valid, or `Some(error_message)` describing
/// the first validation failure encountered.
///
/// `market_open` should reflect `app.clock.as_ref().map(|c| c.is_open).unwrap_or(true)`.
/// DAY orders are blocked when the market is closed; GTC orders are always allowed.
pub(crate) fn validate(
    state: &OrderEntryState,
    buying_power: f64,
    market_open: bool,
) -> Option<String> {
    // 1. Symbol must be non-empty.
    if state.symbol.trim().is_empty() {
        return Some("Symbol cannot be empty".into());
    }

    // 2. Block DAY orders when market is closed.
    if !market_open && !state.gtc_order {
        return Some("Market is closed — switch to GTC or wait for market open".into());
    }

    // 3. Quantity must be a positive number (if provided; an empty qty means
    //    notional dollar amount, which requires a non-empty price instead).
    let qty: Option<f64> = if state.qty_input.is_empty() {
        None
    } else {
        match state.qty_input.parse::<f64>() {
            Ok(v) if v > 0.0 => Some(v),
            Ok(_) => return Some("Quantity must be greater than zero".into()),
            Err(_) => return Some("Quantity is not a valid number".into()),
        }
    };

    // 4. Price must be a positive number on LIMIT orders.
    let price: Option<f64> = if state.market_order {
        None
    } else {
        match state.price_input.parse::<f64>() {
            Ok(v) if v > 0.0 => Some(v),
            Ok(_) => return Some("Price must be greater than zero".into()),
            Err(_) => return Some("Price is not a valid number for a LIMIT order".into()),
        }
    };

    // 5. Estimated total must not exceed buying power.
    //    est_total = qty * price (LIMIT) or qty alone can't be checked for MARKET;
    //    we only gate when we have both values.
    if let (Some(q), Some(p)) = (qty, price) {
        let est_total = q * p;
        if est_total > buying_power {
            return Some(format!(
                "Order total ${:.2} exceeds buying power ${:.2}",
                est_total, buying_power
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::OrderEntryState;

    fn base_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.market_order = false; // LIMIT
        s.gtc_order = false; // DAY
        s.qty_input = "10".into();
        s.price_input = "150.00".into();
        s
    }

    #[test]
    fn valid_limit_order_returns_none() {
        let state = base_state();
        assert_eq!(validate(&state, 10_000.0, true), None);
    }

    #[test]
    fn valid_market_order_returns_none() {
        let mut state = base_state();
        state.market_order = true;
        state.price_input.clear(); // price not required for MARKET
        assert_eq!(validate(&state, 10_000.0, true), None);
    }

    #[test]
    fn empty_symbol_fails() {
        let mut state = base_state();
        state.symbol.clear();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn whitespace_only_symbol_fails() {
        let mut state = base_state();
        state.symbol = "   ".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn zero_qty_fails() {
        let mut state = base_state();
        state.qty_input = "0".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn negative_qty_fails() {
        let mut state = base_state();
        state.qty_input = "-5".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn non_numeric_qty_fails() {
        let mut state = base_state();
        state.qty_input = "abc".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn empty_qty_is_allowed_as_notional() {
        let mut state = base_state();
        state.qty_input.clear(); // notional; price still required for LIMIT
                                 // price is set and positive → should pass
        assert_eq!(validate(&state, 10_000.0, true), None);
    }

    #[test]
    fn zero_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "0".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn negative_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "-1.0".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn non_numeric_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "abc".into();
        assert!(validate(&state, 10_000.0, true).is_some());
    }

    #[test]
    fn price_not_required_for_market_order() {
        let mut state = base_state();
        state.market_order = true;
        state.price_input = "not-a-number".into(); // ignored for MARKET
        assert_eq!(validate(&state, 10_000.0, true), None);
    }

    #[test]
    fn total_exceeding_buying_power_fails() {
        let state = base_state(); // 10 shares × $150 = $1500
        assert!(validate(&state, 1_000.0, true).is_some());
    }

    #[test]
    fn total_exactly_at_buying_power_passes() {
        let state = base_state(); // 10 × 150 = 1500
        assert_eq!(validate(&state, 1_500.0, true), None);
    }

    #[test]
    fn error_message_contains_amounts_when_exceeding_buying_power() {
        let state = base_state(); // 10 × 150 = $1500
        let msg = validate(&state, 500.0, true).expect("should fail");
        assert!(msg.contains("1500.00"), "got: {msg}");
        assert!(msg.contains("500.00"), "got: {msg}");
    }

    // ── Market-closed checks ──────────────────────────────────────────────────

    #[test]
    fn day_order_when_market_closed_fails() {
        let mut state = base_state();
        state.gtc_order = false; // DAY
        let msg = validate(&state, 10_000.0, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("closed") || msg.to_lowercase().contains("gtc"),
            "expected closed/GTC mention, got: {msg}"
        );
    }

    #[test]
    fn gtc_order_when_market_closed_passes() {
        let mut state = base_state();
        state.gtc_order = true; // GTC — valid outside market hours
        assert_eq!(validate(&state, 10_000.0, false), None);
    }

    #[test]
    fn day_order_when_market_open_passes() {
        let state = base_state(); // DAY, market open
        assert_eq!(validate(&state, 10_000.0, true), None);
    }

    #[test]
    fn market_closed_error_checked_before_other_errors() {
        // Even with a bad qty, the market-closed error should surface first
        let mut state = base_state();
        state.gtc_order = false;
        state.qty_input = "-99".into();
        let msg = validate(&state, 10_000.0, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("closed") || msg.to_lowercase().contains("gtc"),
            "market-closed check should run before qty check; got: {msg}"
        );
    }
}
