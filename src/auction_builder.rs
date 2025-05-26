use crate::auction::{Order, OrderId, OrderType, Participant, ParticipantId, ResourceId};
use crate::types::{OrderRequest, ResourceTypeExt, VillageId};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Builder for creating auction orders with a cleaner API
pub struct AuctionBuilder {
    orders: Vec<Order>,
    participants: HashMap<ParticipantId, Participant>,
    order_counter: usize,
    timestamp_counter: u64,
}

impl AuctionBuilder {
    pub fn new() -> Self {
        Self {
            orders: Vec::new(),
            participants: HashMap::new(),
            order_counter: 0,
            timestamp_counter: 0,
        }
    }
    
    /// Register a village as a participant
    pub fn add_village(&mut self, village_id: &VillageId, budget: Decimal) {
        let participant_id = ParticipantId(village_id.to_participant_id());
        self.participants.insert(
            participant_id.clone(),
            Participant {
                id: participant_id,
                currency: budget,
            },
        );
    }
    
    /// Add an order from a village
    pub fn add_order(&mut self, village_id: &VillageId, request: OrderRequest) {
        let order = Order {
            id: OrderId(self.order_counter),
            participant_id: ParticipantId(village_id.to_participant_id()),
            resource_id: ResourceId(request.resource.as_str().to_string()),
            order_type: if request.is_buy { OrderType::Bid } else { OrderType::Ask },
            original_quantity: request.quantity as u64,
            effective_quantity: request.quantity as u64,
            limit_price: request.price,
            timestamp: self.timestamp_counter,
        };
        
        self.orders.push(order);
        self.order_counter += 1;
        self.timestamp_counter += 1;
    }
    
    /// Get the built orders and participants
    pub fn build(self) -> (Vec<Order>, HashMap<ParticipantId, Participant>) {
        (self.orders, self.participants)
    }
}

impl Default for AuctionBuilder {
    fn default() -> Self {
        Self::new()
    }
}