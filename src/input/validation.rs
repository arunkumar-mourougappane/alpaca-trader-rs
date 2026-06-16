use crate::app::{FullOrderType, OrderEntryState, TrailType};

/// Validates an [`OrderEntryState`] before a submit command is dispatched.
///
/// Returns `None` when the state is valid, or `Some(error_message)` describing
/// the first validation failure encountered.
///
/// `market_open` reflects `app.clock.as_ref().map(|c| c.is_open).unwrap_or(true)`.
/// `extended_hours_ok` is `true` when the market is in pre- or after-hours
/// (i.e. `MarketState::PreMarket` or `MarketState::AfterHours`).
/// DAY orders are blocked when the market is closed; GTC orders are always allowed.
/// Extended-hours limit orders are allowed when `extended_hours_ok` is `true`.
pub(crate) fn validate(
    state: &OrderEntryState,
    buying_power: f64,
    market_open: bool,
    extended_hours_ok: bool,
) -> Option<String> {
    // 1. Symbol must be non-empty.
    if state.symbol.trim().is_empty() {
        return Some("Symbol cannot be empty".into());
    }

    // 2. Block DAY orders when market is closed, unless extended-hours limit is active.
    if !market_open && !state.gtc_order {
        let ext_hours_allowed =
            state.extended_hours && extended_hours_ok && state.order_type == FullOrderType::Limit;
        if !ext_hours_allowed {
            return Some("Market is closed — switch to GTC or wait for market open".into());
        }
    }

    // 3. Extended-hours validation.
    if state.extended_hours {
        if state.order_type != FullOrderType::Limit {
            return Some("Extended hours is only available for limit orders".into());
        }
        if state.gtc_order {
            return Some("Extended hours requires DAY time-in-force".into());
        }
    }

    // 4. Quantity must be a positive number (if provided).
    let qty: Option<f64> = if state.qty_input.is_empty() {
        None
    } else {
        match state.qty_input.parse::<f64>() {
            Ok(v) if v > 0.0 => Some(v),
            Ok(_) => return Some("Quantity must be greater than zero".into()),
            Err(_) => return Some("Quantity is not a valid number".into()),
        }
    };

    // 5. Limit price — required for Limit and StopLimit orders.
    let price: Option<f64> = match state.order_type {
        FullOrderType::Limit | FullOrderType::StopLimit => match state.price_input.parse::<f64>() {
            Ok(v) if v > 0.0 => Some(v),
            Ok(_) => return Some("Price must be greater than zero".into()),
            Err(_) => return Some("Price is not a valid number for a LIMIT order".into()),
        },
        _ => None,
    };

    // 6. Stop price — required for Stop and StopLimit orders.
    if matches!(
        state.order_type,
        FullOrderType::Stop | FullOrderType::StopLimit
    ) {
        match state.stop_price_input.parse::<f64>() {
            Ok(v) if v > 0.0 => {}
            Ok(_) => return Some("Stop price must be greater than zero".into()),
            Err(_) => return Some("Stop price is not a valid number".into()),
        }
    }

    // 7. Trail amount — required for TrailingStop orders.
    if state.order_type == FullOrderType::TrailingStop {
        match state.trail_input.parse::<f64>() {
            Ok(v) if v > 0.0 => {}
            Ok(_) => {
                let unit = if state.trail_type == TrailType::Percent {
                    "%"
                } else {
                    "$"
                };
                return Some(format!("Trail amount ({unit}) must be greater than zero"));
            }
            Err(_) => return Some("Trail amount is not a valid number".into()),
        }
    }

    // 8. Estimated total must not exceed buying power (limit orders only).
    if let (Some(q), Some(p)) = (qty, price) {
        let est_total = q * p;
        if est_total > buying_power {
            return Some(format!(
                "Order total ${:.2} exceeds buying power ${:.2}",
                est_total, buying_power
            ));
        }
    }

    // 9. Bracket order validation.
    if state.bracket {
        use crate::app::OrderSide;
        if state.side == OrderSide::SellShort {
            return Some("Bracket orders are not available for sell-short positions".into());
        }
        if !matches!(
            state.order_type,
            FullOrderType::Market | FullOrderType::Limit
        ) {
            return Some("Bracket orders require a Market or Limit entry type".into());
        }
        let tp: f64 = match state.take_profit_price.parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            Ok(_) => return Some("Take-profit price must be greater than zero".into()),
            Err(_) => return Some("Take-profit price is not a valid number".into()),
        };
        let sl: f64 = match state.stop_loss_price.parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            Ok(_) => return Some("Stop-loss price must be greater than zero".into()),
            Err(_) => return Some("Stop-loss price is not a valid number".into()),
        };
        if !state.stop_loss_limit_price.is_empty() {
            match state.stop_loss_limit_price.parse::<f64>() {
                Ok(v) if v > 0.0 => {}
                Ok(_) => return Some("Stop-loss limit price must be greater than zero".into()),
                Err(_) => return Some("Stop-loss limit price is not a valid number".into()),
            }
        }
        // Directional validation: for buy orders TP must be above SL.
        if state.side == OrderSide::Buy {
            if tp <= sl {
                return Some("Take-profit must be above stop-loss for a buy bracket order".into());
            }
            // If we have a limit entry price, TP must be above it and SL must be below.
            if let Ok(entry) = state.price_input.parse::<f64>() {
                if entry > 0.0 && tp <= entry {
                    return Some("Take-profit must be above the limit entry price".into());
                }
                if entry > 0.0 && sl >= entry {
                    return Some("Stop-loss must be below the limit entry price".into());
                }
            }
        } else {
            // Sell bracket: TP below SL.
            if tp >= sl {
                return Some("Take-profit must be below stop-loss for a sell bracket order".into());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{FullOrderType, OrderEntryState, TrailType};

    fn base_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Limit;
        s.gtc_order = false; // DAY
        s.qty_input = "10".into();
        s.price_input = "150.00".into();
        s
    }

    fn base_market_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.gtc_order = false;
        s.qty_input = "10".into();
        s
    }

    fn base_stop_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Stop;
        s.gtc_order = false;
        s.qty_input = "10".into();
        s.stop_price_input = "145.00".into();
        s
    }

    fn base_stop_limit_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::StopLimit;
        s.gtc_order = false;
        s.qty_input = "10".into();
        s.stop_price_input = "145.00".into();
        s.price_input = "144.00".into();
        s
    }

    fn base_trailing_stop_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::TrailingStop;
        s.gtc_order = false;
        s.qty_input = "10".into();
        s.trail_input = "5.00".into();
        s.trail_type = TrailType::Price;
        s
    }

    #[test]
    fn valid_limit_order_returns_none() {
        let state = base_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn valid_market_order_returns_none() {
        let state = base_market_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn empty_symbol_fails() {
        let mut state = base_state();
        state.symbol.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn whitespace_only_symbol_fails() {
        let mut state = base_state();
        state.symbol = "   ".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn zero_qty_fails() {
        let mut state = base_state();
        state.qty_input = "0".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn negative_qty_fails() {
        let mut state = base_state();
        state.qty_input = "-5".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn non_numeric_qty_fails() {
        let mut state = base_state();
        state.qty_input = "abc".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn empty_qty_is_allowed_as_notional() {
        let mut state = base_state();
        state.qty_input.clear();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn zero_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "0".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn negative_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "-1.0".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn non_numeric_price_on_limit_fails() {
        let mut state = base_state();
        state.price_input = "abc".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn price_not_required_for_market_order() {
        let state = base_market_state();
        // price_input is ignored for MARKET orders
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn total_exceeding_buying_power_fails() {
        let state = base_state(); // 10 shares × $150 = $1500
        assert!(validate(&state, 1_000.0, true, false).is_some());
    }

    #[test]
    fn total_exactly_at_buying_power_passes() {
        let state = base_state(); // 10 × 150 = 1500
        assert_eq!(validate(&state, 1_500.0, true, false), None);
    }

    #[test]
    fn error_message_contains_amounts_when_exceeding_buying_power() {
        let state = base_state(); // 10 × 150 = $1500
        let msg = validate(&state, 500.0, true, false).expect("should fail");
        assert!(msg.contains("1500.00"), "got: {msg}");
        assert!(msg.contains("500.00"), "got: {msg}");
    }

    // ── Market-closed checks ──────────────────────────────────────────────────

    #[test]
    fn day_order_when_market_closed_fails() {
        let mut state = base_state();
        state.gtc_order = false; // DAY
        let msg = validate(&state, 10_000.0, false, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("closed") || msg.to_lowercase().contains("gtc"),
            "expected closed/GTC mention, got: {msg}"
        );
    }

    #[test]
    fn gtc_order_when_market_closed_passes() {
        let mut state = base_state();
        state.gtc_order = true; // GTC — valid outside market hours
        assert_eq!(validate(&state, 10_000.0, false, false), None);
    }

    #[test]
    fn day_order_when_market_open_passes() {
        let state = base_state(); // DAY, market open
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn market_closed_error_checked_before_other_errors() {
        let mut state = base_state();
        state.gtc_order = false;
        state.qty_input = "-99".into();
        let msg = validate(&state, 10_000.0, false, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("closed") || msg.to_lowercase().contains("gtc"),
            "market-closed check should run before qty check; got: {msg}"
        );
    }

    // ── Stop order ────────────────────────────────────────────────────────────

    #[test]
    fn valid_stop_order_returns_none() {
        let state = base_stop_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn stop_order_missing_stop_price_fails() {
        let mut state = base_stop_state();
        state.stop_price_input.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn stop_order_zero_stop_price_fails() {
        let mut state = base_stop_state();
        state.stop_price_input = "0".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn stop_order_invalid_stop_price_fails() {
        let mut state = base_stop_state();
        state.stop_price_input = "abc".into();
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("stop"),
            "error should mention stop; got: {msg}"
        );
    }

    // ── Stop-limit order ──────────────────────────────────────────────────────

    #[test]
    fn valid_stop_limit_order_returns_none() {
        let state = base_stop_limit_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn stop_limit_missing_limit_price_fails() {
        let mut state = base_stop_limit_state();
        state.price_input.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn stop_limit_missing_stop_price_fails() {
        let mut state = base_stop_limit_state();
        state.stop_price_input.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    // ── Trailing stop order ───────────────────────────────────────────────────

    #[test]
    fn valid_trailing_stop_dollar_returns_none() {
        let state = base_trailing_stop_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn valid_trailing_stop_percent_returns_none() {
        let mut state = base_trailing_stop_state();
        state.trail_type = TrailType::Percent;
        state.trail_input = "2.0".into();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn trailing_stop_missing_trail_amount_fails() {
        let mut state = base_trailing_stop_state();
        state.trail_input.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn trailing_stop_zero_trail_amount_fails() {
        let mut state = base_trailing_stop_state();
        state.trail_input = "0".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn trailing_stop_invalid_trail_amount_fails() {
        let mut state = base_trailing_stop_state();
        state.trail_input = "bad".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn trailing_stop_zero_percent_trail_fails() {
        let mut state = base_trailing_stop_state();
        state.trail_type = TrailType::Percent;
        state.trail_input = "0".into();
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(msg.contains('%'), "error should mention % unit; got: {msg}");
    }

    // ── Extended hours ────────────────────────────────────────────────────────

    #[test]
    fn extended_hours_limit_day_in_premarket_passes() {
        let mut state = base_state();
        state.extended_hours = true;
        state.gtc_order = false;
        assert_eq!(validate(&state, 10_000.0, false, true), None);
    }

    #[test]
    fn extended_hours_limit_day_when_fully_closed_fails() {
        let mut state = base_state();
        state.extended_hours = true;
        state.gtc_order = false;
        // extended_hours_ok = false → fully closed
        let msg = validate(&state, 10_000.0, false, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("closed") || msg.to_lowercase().contains("gtc"),
            "got: {msg}"
        );
    }

    #[test]
    fn extended_hours_on_non_limit_order_fails() {
        let mut state = base_stop_state();
        state.extended_hours = true;
        let msg = validate(&state, 10_000.0, true, true).expect("should fail");
        assert!(msg.to_lowercase().contains("extended"), "got: {msg}");
    }

    #[test]
    fn extended_hours_with_gtc_fails() {
        let mut state = base_state();
        state.extended_hours = true;
        state.gtc_order = true;
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("extended") || msg.to_lowercase().contains("day"),
            "got: {msg}"
        );
    }

    // ── Bracket order ─────────────────────────────────────────────────────────

    fn base_bracket_state() -> OrderEntryState {
        let mut s = OrderEntryState::new("AAPL".into());
        s.order_type = FullOrderType::Market;
        s.gtc_order = true;
        s.qty_input = "10".into();
        s.bracket = true;
        s.take_profit_price = "185.00".into();
        s.stop_loss_price = "165.00".into();
        s
    }

    #[test]
    fn valid_bracket_market_buy_returns_none() {
        let state = base_bracket_state();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn valid_bracket_limit_buy_returns_none() {
        let mut state = base_bracket_state();
        state.order_type = FullOrderType::Limit;
        state.price_input = "175.00".into();
        assert_eq!(validate(&state, 100_000.0, true, false), None);
    }

    #[test]
    fn bracket_sell_short_fails() {
        use crate::app::OrderSide;
        let mut state = base_bracket_state();
        state.side = OrderSide::SellShort;
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("bracket") || msg.to_lowercase().contains("short"),
            "got: {msg}"
        );
    }

    #[test]
    fn bracket_trailing_stop_fails() {
        let mut state = base_bracket_state();
        state.order_type = FullOrderType::TrailingStop;
        state.trail_input = "5.00".into();
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(msg.to_lowercase().contains("bracket"), "got: {msg}");
    }

    #[test]
    fn bracket_missing_take_profit_fails() {
        let mut state = base_bracket_state();
        state.take_profit_price.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn bracket_missing_stop_loss_fails() {
        let mut state = base_bracket_state();
        state.stop_loss_price.clear();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }

    #[test]
    fn bracket_buy_tp_below_sl_fails() {
        let mut state = base_bracket_state();
        // Inverted: TP below SL for a buy order
        state.take_profit_price = "160.00".into();
        state.stop_loss_price = "170.00".into();
        let msg = validate(&state, 10_000.0, true, false).expect("should fail");
        assert!(
            msg.to_lowercase().contains("take-profit") || msg.to_lowercase().contains("stop-loss"),
            "got: {msg}"
        );
    }

    #[test]
    fn bracket_buy_tp_above_entry_sl_below_entry_passes() {
        let mut state = base_bracket_state();
        state.order_type = FullOrderType::Limit;
        state.price_input = "175.00".into();
        state.take_profit_price = "185.00".into();
        state.stop_loss_price = "165.00".into();
        assert_eq!(validate(&state, 100_000.0, true, false), None);
    }

    #[test]
    fn bracket_buy_tp_below_limit_entry_fails() {
        let mut state = base_bracket_state();
        state.order_type = FullOrderType::Limit;
        state.price_input = "175.00".into();
        state.take_profit_price = "170.00".into(); // below entry
        state.stop_loss_price = "160.00".into();
        let msg = validate(&state, 100_000.0, true, false).expect("should fail");
        assert!(msg.to_lowercase().contains("take-profit"), "got: {msg}");
    }

    #[test]
    fn bracket_buy_sl_above_limit_entry_fails() {
        let mut state = base_bracket_state();
        state.order_type = FullOrderType::Limit;
        state.price_input = "175.00".into();
        state.take_profit_price = "185.00".into();
        state.stop_loss_price = "180.00".into(); // above entry
        let msg = validate(&state, 100_000.0, true, false).expect("should fail");
        assert!(msg.to_lowercase().contains("stop-loss"), "got: {msg}");
    }

    #[test]
    fn bracket_optional_sl_limit_price_valid() {
        let mut state = base_bracket_state();
        state.stop_loss_limit_price = "164.00".into();
        assert_eq!(validate(&state, 10_000.0, true, false), None);
    }

    #[test]
    fn bracket_invalid_sl_limit_price_fails() {
        let mut state = base_bracket_state();
        state.stop_loss_limit_price = "bad".into();
        assert!(validate(&state, 10_000.0, true, false).is_some());
    }
}
