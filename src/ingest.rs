#[derive(Debug)]
pub struct BinanceFillMessage {
    pub trade_id: u64,
    pub symbol: String,
    pub price: f64,
    pub quantity: f64,
    pub side: String,
    pub timestamp: u64,
}
