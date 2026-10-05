use std::collections::HashMap;

use crate::domain::{Event, EventKind, EventSide};

pub struct PositionState {
    positions: HashMap<String, f64>,
}

impl PositionState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    pub fn apply(&mut self, event: &Event) {
        match &event.kind {
            EventKind::Fill(fill) => {
                let position = self.positions.entry(fill.instrument.clone()).or_insert(0.0);
                match fill.side {
                    EventSide::Buy => {
                        *position += fill.quantity;
                    }

                    EventSide::Sell => {
                        *position -= fill.quantity;
                    }
                }
            }
        }
    }
    pub fn get(&self, instrument: &str) -> f64 {
        match self.positions.get(instrument) {
            Some(position) => *position,
            None => 0.0,
        }
    }
}
