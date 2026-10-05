use crate::domain::Source::Binance;
use crate::domain::{Event, EventKind, EventSide, Fill, Source};
use crate::ingest::BinanceFillMessage;
#[derive(Debug)]
pub enum NormalizeError {
    InvalidSide(String),
}
fn string_to_side(input: String) -> Result<EventSide, NormalizeError> {
    let side = match input.as_str() {
        "BUY" => EventSide::Buy,
        "SELL" => EventSide::Sell,
        _ => return Err(NormalizeError::InvalidSide(input)),
    };
    Ok(side)
}

pub fn normalise_binance_fill(fill: BinanceFillMessage) -> Result<Event, NormalizeError> {
    let side = string_to_side(fill.side)?;
    let internal_fill = Fill {
        instrument: fill.symbol,
        side,
        quantity: fill.quantity,
        price: fill.price,
    };
    Ok(Event {
        source: Source::Binance,
        source_id: fill.trade_id.to_string(),
        timestamp: fill.timestamp,
        kind: EventKind::Fill(internal_fill),
    })
}
