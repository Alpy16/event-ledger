use crate::dedupe::Deduplicator;
use crate::ingest::BinanceFillMessage;
use crate::ledger::Ledger;
use crate::normalize::normalise_binance_fill;
use crate::position::PositionState;
use crate::validation::{ValidationError, validate_event};

use std::path::Path;

mod dedupe;
mod domain;
mod ingest;
mod ledger;
mod normalize;
mod position;
mod validation;

fn main() {
    let mut ledger = Ledger::open(Path::new("ledger.txt")).expect("Failed to open ledger");
    let recovered_events = ledger.replay().expect("Failed to replay ledger");
    let mut positions = PositionState::new();

    for event in &recovered_events {
        positions.apply(event);
        println!("Recovered BTCUSDT position: {}", positions.get("BTCUSDT"));
    }

    let mut deduplicator = Deduplicator::new();
    for event in &recovered_events {
        deduplicator.insert(event);
    }

    println!("Recovered {} events", recovered_events.len());

    for event in &recovered_events {
        println!("Recovered: {:?}", event);
    }

    let message = BinanceFillMessage {
        trade_id: 12345,
        symbol: "BTCUSDT".to_string(),
        price: 50000.0,
        quantity: 0.1,
        side: "BUY".to_string(),
        timestamp: 1620000000,
    };

    match normalise_binance_fill(message) {
        Ok(event) => {
            let validation_result = validate_event(event);

            match validation_result {
                Ok(valid_event) => {
                    println!("Validated event: {:?}", valid_event);

                    if deduplicator.contains(&valid_event) {
                        println!("Duplicate event ignored");
                    } else {
                        match ledger.append(&valid_event) {
                            Ok(()) => {
                                deduplicator.insert(&valid_event);
                                println!("Event appended to ledger");
                            }

                            Err(e) => {
                                println!("Ledger append failed: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Validation failed: {:?}", e);
                }
            }
        }

        Err(e) => {
            println!("Normalization failed: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests;
