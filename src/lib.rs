use std::collections::HashMap;

use auction::{
    AuctionError, AuctionSuccess, Order, OrderId, OrderType, Participant, ParticipantId,
    ResourceId, run_auction,
};
use rust_decimal::Decimal;

pub mod auction;
pub mod fp;
pub mod old_auction;

// Public Auction struct for use from main
#[derive(Debug)]
pub struct Auction {
    pub orders: Vec<Order>,
    pub participants: HashMap<ParticipantId, Participant>,
    pub max_iterations: u32,
    pub last_clearing_prices: HashMap<ResourceId, Decimal>,
}

impl Auction {
    pub fn new(max_iterations: u32) -> Self {
        Auction {
            orders: Vec::new(),
            participants: HashMap::new(),
            max_iterations,
            last_clearing_prices: HashMap::new(),
        }
    }

    pub fn add_order(
        &mut self,
        id: usize,
        p_id: &str,
        r_id: &str,
        order_type: OrderType,
        qty: u64,
        price: Decimal,
        ts: u64,
    ) {
        let order = Order {
            id: OrderId(id),
            participant_id: ParticipantId(p_id.parse().expect("Invalid participant ID")),
            resource_id: ResourceId(r_id.to_string()),
            order_type,
            original_quantity: qty,
            effective_quantity: qty,
            limit_price: price,
            timestamp: ts,
        };
        self.orders.push(order);
    }

    pub fn add_participant(&mut self, id_str: &str, currency: Decimal) {
        let id = ParticipantId(id_str.parse().expect("Invalid participant ID"));
        let participant = Participant {
            id: id.clone(),
            currency,
        };
        self.participants.insert(id, participant);
    }

    pub fn run(&self) -> Result<AuctionSuccess, AuctionError> {
        run_auction(
            self.orders.clone(),
            self.participants.clone(),
            self.max_iterations,
            self.last_clearing_prices.clone(),
        )
    }
}
