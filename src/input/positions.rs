use crossterm::event::KeyCode;

use crate::app::{App, Modal, OrderEntryState, OrderSide};

pub(crate) fn handle_positions_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.positions.len();

    super::handle_nav_key(
        key.code,
        len,
        &mut app.positions_state,
        &mut app.pending_g_at,
    );

    match key.code {
        KeyCode::Enter => {
            if let Some(symbol) = app.selected_position_symbol() {
                let _ = app
                    .command_tx
                    .try_send(crate::commands::Command::FetchIntradayBars(symbol.clone()));
                app.modal = Some(Modal::SymbolDetail(symbol));
            }
        }
        KeyCode::Char('o') => {
            let symbol = app.selected_position_symbol().unwrap_or_default();
            app.modal = Some(Modal::OrderEntry(
                OrderEntryState::new(symbol).with_side(OrderSide::Sell),
            ));
        }
        KeyCode::Char('s') => {
            app.positions_sort.col = app.positions_sort.col.cycle();
        }
        KeyCode::Char('S') => {
            app.positions_sort.toggle_dir();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::test_helpers::make_test_app;
    use crate::app::{PositionSortCol, SortDir};
    use crate::types::Position;

    fn press(app: &mut crate::app::App, code: KeyCode) {
        let event = KeyEvent::new(code, KeyModifiers::NONE);
        super::handle_positions_key(app, event);
    }

    fn make_position(symbol: &str) -> Position {
        Position {
            symbol: symbol.into(),
            qty: "10".into(),
            avg_entry_price: "100.00".into(),
            current_price: "110.00".into(),
            market_value: "1100.00".into(),
            unrealized_pl: "100.00".into(),
            unrealized_plpc: "0.10".into(),
            side: "long".into(),
            asset_class: "us_equity".into(),
        }
    }

    #[test]
    fn s_key_cycles_positions_sort_column() {
        let mut app = make_test_app();
        app.positions.push(make_position("AAPL"));
        assert_eq!(app.positions_sort.col, PositionSortCol::None);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.positions_sort.col, PositionSortCol::Symbol);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.positions_sort.col, PositionSortCol::Qty);
    }

    #[test]
    fn shift_s_toggles_positions_sort_direction() {
        let mut app = make_test_app();
        assert_eq!(app.positions_sort.dir, SortDir::Asc);
        let event = crossterm::event::KeyEvent::new(
            KeyCode::Char('S'),
            crossterm::event::KeyModifiers::SHIFT,
        );
        super::handle_positions_key(&mut app, event);
        assert_eq!(app.positions_sort.dir, SortDir::Desc);
    }

    #[test]
    fn s_key_wraps_back_to_none_after_full_cycle() {
        let mut app = make_test_app();
        // cycle through all 6 sortable columns + back to None
        for _ in 0..7 {
            press(&mut app, KeyCode::Char('s'));
        }
        assert_eq!(app.positions_sort.col, PositionSortCol::None);
    }

    #[test]
    fn o_key_opens_sell_order_modal() {
        let mut app = make_test_app();
        app.positions.push(make_position("TSLA"));
        app.positions_state.select(Some(0));
        press(&mut app, KeyCode::Char('o'));
        assert!(app.modal.is_some(), "expected modal after pressing o");
    }
}
