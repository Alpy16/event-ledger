use crate::domain::{Event, EventKind};

#[derive(Debug)]
pub enum ValidationError {
    InvalidQuantity(f64),
    InvalidPrice(f64),
    InvalidInstrument,
}
pub fn validate_event(event: Event) -> Result<Event, ValidationError> {
    match &event.kind {
        EventKind::Fill(fill) => {
            if fill.quantity <= 0.0 {
                return Err(ValidationError::InvalidQuantity(fill.quantity));
            }

            if fill.price <= 0.0 {
                return Err(ValidationError::InvalidPrice(fill.price));
            }

            if fill.instrument.is_empty() {
                return Err(ValidationError::InvalidInstrument);
            }
        }
    }
    Ok(event)
}
