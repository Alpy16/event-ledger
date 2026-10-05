use crate::domain::{EventKind, EventSide, Source};
use crate::ingest::BinanceFillMessage;
use crate::normalize::{NormalizeError, normalise_binance_fill};
use crate::validation::{ValidationError, validate_event};

#[test]
fn valid_binance_fill_is_normalized_and_validated() {
    let message = BinanceFillMessage {
        trade_id: 12345,
        symbol: "BTCUSDT".to_string(),
        price: 50_000.0,
        quantity: 0.1,
        side: "BUY".to_string(),
        timestamp: 1_620_000_000,
    };

    let event = normalise_binance_fill(message).expect("normalization should succeed");

    let event = validate_event(event).expect("validation should succeed");

    assert_eq!(event.source, Source::Binance);
    assert_eq!(event.source_id, "12345");

    match event.kind {
        EventKind::Fill(fill) => {
            assert_eq!(fill.instrument, "BTCUSDT");
            assert_eq!(fill.side, EventSide::Buy);
            assert_eq!(fill.quantity, 0.1);
            assert_eq!(fill.price, 50_000.0);
        }
    }
}

#[test]
fn invalid_side_fails_normalization() {
    let message = BinanceFillMessage {
        trade_id: 12345,
        symbol: "BTCUSDT".to_string(),
        price: 50_000.0,
        quantity: 0.1,
        side: "BANANA".to_string(),
        timestamp: 1_620_000_000,
    };

    let result = normalise_binance_fill(message);

    assert!(matches!(result, Err(NormalizeError::InvalidSide(_))));
}

#[test]
fn negative_quantity_fails_validation() {
    let message = BinanceFillMessage {
        trade_id: 12345,
        symbol: "BTCUSDT".to_string(),
        price: 50_000.0,
        quantity: -0.1,
        side: "BUY".to_string(),
        timestamp: 1_620_000_000,
    };

    let event = normalise_binance_fill(message).expect("normalization should succeed");

    let result = validate_event(event);

    assert!(matches!(result, Err(ValidationError::InvalidQuantity(_))));
}
