use rust_decimal::prelude::*; // Includes Decimal, Zero, One, FromPrimitive, ToPrimitive
use rust_decimal_macros::dec; // For the dec! macro
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

// --- Data Structures (IDs, OrderType remain the same) ---

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParticipantId(pub u32);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Bid, // Buy
    Ask, // Sell
}

// --- Updated Structures using Decimal ---

#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub participant_id: ParticipantId,
    pub resource_id: ResourceId,
    pub order_type: OrderType,
    pub original_quantity: u64,
    pub effective_quantity: u64, // Quantity used in matching, potentially reduced by pruning
    pub limit_price: Decimal,    // <-- Use Decimal for price
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct Participant {
    pub id: ParticipantId,
    pub currency: Decimal, // <-- Use Decimal for currency
}

// Represents a filled portion of an order in a specific iteration
#[derive(Debug, Clone, Copy)]
pub struct TentativeFill {
    pub order_id: OrderId,
    pub filled_quantity: u64,
}

// Result of clearing a single resource
#[derive(Debug, Clone)]
pub struct ResourceClearing {
    pub clearing_price: Decimal, // <-- Use Decimal
    pub matched_volume: u64,
    pub tentative_fills: Vec<TentativeFill>,
}

// --- Public API Structures (using Decimal) ---

#[derive(Debug, Clone)]
pub struct FinalFill {
    pub order_id: OrderId,
    pub participant_id: ParticipantId,
    pub resource_id: ResourceId,
    pub order_type: OrderType,
    pub filled_quantity: u64,
    pub price: Decimal, // <-- Use Decimal
}

#[derive(Debug, Clone)]
pub struct FinalBalance {
    pub participant_id: ParticipantId,
    pub final_currency: Decimal, // <-- Use Decimal
}

#[derive(Debug, Clone)]
pub struct AuctionSuccess {
    pub final_fills: Vec<FinalFill>,
    pub final_balances: Vec<FinalBalance>,
    pub clearing_prices: HashMap<ResourceId, Decimal>, // <-- Use Decimal
}

#[derive(Debug)]
pub enum AuctionError {
    MaxIterationsReached,
    InternalError(String),
}

impl fmt::Display for AuctionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuctionError::MaxIterationsReached => write!(f, "Maximum iterations reached"),
            AuctionError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl Error for AuctionError {}

// --- Auction Logic (Updated for Decimal) ---

pub fn run_auction(
    orders: Vec<Order>,
    participants: HashMap<ParticipantId, Participant>,
    max_iterations: u32,
    last_clearing_prices: HashMap<ResourceId, Decimal>, // <-- Use Decimal
) -> Result<AuctionSuccess, AuctionError> {
    let mut current_orders = orders.clone(); // Orders whose effective_quantity might be pruned
    let mut current_participants = participants.clone();
    // Build order_map once for efficient lookup
    let mut order_map: HashMap<OrderId, Order> =
        current_orders.iter().cloned().map(|o| (o.id, o)).collect();

    for _iteration in 0..max_iterations {
        // println!("--- Iteration {} ---", iteration + 1); // Keep for debugging if needed

        let mut iteration_clearings: HashMap<ResourceId, ResourceClearing> = HashMap::new();
        let mut resource_orders: HashMap<ResourceId, Vec<&Order>> = HashMap::new();

        // 1. Group orders by resource (using current effective quantities)
        for order in current_orders.iter() {
            if order.effective_quantity > 0 {
                resource_orders
                    .entry(order.resource_id.clone())
                    .or_default()
                    .push(order);
            }
        }

        // 2. & 3. Build Curves, Find Clearing Price & Tentative Fills for each resource
        for (resource_id, orders_for_resource) in resource_orders {
            // Pass order_map by reference
            match find_clearing_for_resource(
                &orders_for_resource,
                last_clearing_prices.get(&resource_id).copied(),
                &order_map,
            ) {
                Ok(Some(clearing)) => {
                    // println!( // Keep for debugging if needed
                    //     "  Resource {:?}: Price={}, Volume={}",
                    //     resource_id, clearing.clearing_price, clearing.matched_volume
                    // );
                    // for fill in &clearing.tentative_fills {
                    //     println!("    Fill: Order {:?}, Qty {}", fill.order_id, fill.filled_quantity);
                    // }
                    iteration_clearings.insert(resource_id.clone(), clearing);
                }
                Ok(None) => {
                    // println!("  Resource {:?}: No clearing possible", resource_id); // Debugging
                }
                Err(e) => return Err(AuctionError::InternalError(e)),
            }
        }

        // 4. Compute Net Outflows
        let mut net_outflows: HashMap<ParticipantId, Decimal> = HashMap::new();
        let mut costs: HashMap<ParticipantId, Decimal> = HashMap::new();
        // Store only needed info for pruning: (OrderID, FilledQty, ClearingPrice)
        let mut tentative_buy_fills_info: HashMap<ParticipantId, Vec<(OrderId, u64, Decimal)>> =
            HashMap::new();

        for (_resource_id, clearing) in &iteration_clearings {
            let price = clearing.clearing_price;
            for fill in &clearing.tentative_fills {
                // Avoid repeated lookups if possible, though map lookup is fast
                let order = match order_map.get(&fill.order_id) {
                    Some(o) => o,
                    None => {
                        return Err(AuctionError::InternalError(format!(
                            "Order {:?} not found in map during outflow calc",
                            fill.order_id
                        )));
                    }
                };
                let participant_id = order.participant_id.clone();

                // Convert quantity to Decimal for calculation
                let quantity_dec = Decimal::from_u64(fill.filled_quantity).ok_or_else(|| {
                    AuctionError::InternalError(format!(
                        "Failed to convert quantity {} to Decimal",
                        fill.filled_quantity
                    ))
                })?;

                let value = quantity_dec * price;

                let outflow_entry = net_outflows
                    .entry(participant_id.clone())
                    .or_insert(Decimal::ZERO);

                match order.order_type {
                    OrderType::Bid => {
                        *outflow_entry += value;
                        *costs.entry(participant_id.clone()).or_insert(Decimal::ZERO) += value;
                        tentative_buy_fills_info
                            .entry(participant_id)
                            .or_default()
                            .push((fill.order_id, fill.filled_quantity, price)); // Store essential info
                    }
                    OrderType::Ask => {
                        *outflow_entry -= value;
                    }
                }
            }
        }

        // 5. Identify and Prune Short Participants
        let mut short_participants_info = Vec::new(); // Store (ParticipantId, Shortfall)
        let participants_to_check = current_participants.clone();
        for (participant_id, participant) in participants_to_check {
            let outflow = net_outflows
                .get(&participant_id)
                .copied()
                .unwrap_or(Decimal::ZERO);
            // println!("  Participant {:?}: Outflow={}, Currency={}", participant_id, outflow, participant.currency); // Debugging
            if outflow > participant.currency {
                let shortfall = outflow - participant.currency;
                // println!("    SHORT! Shortfall={}", shortfall); // Debugging
                short_participants_info.push((participant_id.clone(), shortfall));
            }
        }

        if short_participants_info.is_empty() {
            // println!("--- Convergence Reached ---"); // Debugging
            // Converged! Prepare Success result
            let mut final_fills = Vec::new();
            let final_clearing_prices = iteration_clearings
                .iter()
                .map(|(rid, rc)| (rid.clone(), rc.clearing_price))
                .collect::<HashMap<_, _>>();

            for (resource_id, clearing) in iteration_clearings {
                let price = clearing.clearing_price;
                for fill in clearing.tentative_fills {
                    // Reuse order lookup logic
                    let order = match order_map.get(&fill.order_id) {
                        Some(o) => o,
                        None => {
                            return Err(AuctionError::InternalError(format!(
                                "Order {:?} not found in map during success fill creation",
                                fill.order_id
                            )));
                        }
                    };
                    final_fills.push(FinalFill {
                        order_id: fill.order_id,
                        participant_id: order.participant_id.clone(),
                        resource_id: resource_id.clone(),
                        order_type: order.order_type,
                        filled_quantity: fill.filled_quantity,
                        price: price, // Already a Decimal
                    });
                }
            }

            // Update balances (using final net_outflows calculated previously)
            for (p_id, outflow) in net_outflows {
                if let Some(p) = current_participants.get_mut(&p_id) {
                    // Check sufficient funds before final debit (should be guaranteed by loop logic, but belt-and-suspenders)
                    if outflow > Decimal::ZERO && outflow > p.currency + dec!(1e-9) {
                        // Allow tiny tolerance just in case
                        return Err(AuctionError::InternalError(format!(
                            "Participant {:?} unexpectedly short ({}) on final settlement (needs {})",
                            p_id, p.currency, outflow
                        )));
                    }
                    p.currency -= outflow; // Apply the net change
                } else {
                    // This shouldn't happen if participants map is consistent
                    return Err(AuctionError::InternalError(format!(
                        "Participant {:?} not found for final balance update",
                        p_id
                    )));
                }
            }
            let final_balances = current_participants
                .values()
                .map(|p| FinalBalance {
                    participant_id: p.id.clone(),
                    final_currency: p.currency,
                })
                .collect();

            // Update last known prices for next potential auction run
            // last_clearing_prices = final_clearing_prices.clone();

            return Ok(AuctionSuccess {
                final_fills,
                final_balances,
                clearing_prices: final_clearing_prices,
            });
        }

        // --- Pruning Logic ---
        // Prune orders for the *next* iteration
        for (participant_id, shortfall) in short_participants_info {
            let total_cost = costs.get(&participant_id).copied().unwrap_or(Decimal::ZERO);

            // Avoid division by zero or pruning if no cost basis
            if total_cost <= Decimal::ZERO {
                continue;
            }
            // Ensure shortfall isn't somehow negative (shouldn't happen)
            if shortfall <= Decimal::ZERO {
                continue;
            }

            // Calculate reduction percentage. Ensure it's capped at 1.0 (100%)
            let reduction_percentage = (shortfall / total_cost).min(Decimal::ONE);
            let reduction_factor = Decimal::ONE - reduction_percentage; // Factor to multiply quantities by

            // println!( // Debugging
            //     "  Pruning Participant {:?}: Shortfall={}, Cost={}, Reduction%={:.2}",
            //     participant_id, shortfall, total_cost, reduction_percentage * dec!(100.0)
            // );

            // Use the collected buy fill info
            if let Some(buy_fills) = tentative_buy_fills_info.get(&participant_id) {
                for (order_id, _filled_qty, _price) in buy_fills {
                    // Find the mutable order in current_orders vec AND the map
                    if let Some(order_to_prune) =
                        current_orders.iter_mut().find(|o| o.id == *order_id)
                    {
                        let original_effective = order_to_prune.effective_quantity;
                        if original_effective == 0 {
                            continue;
                        } // Already fully pruned

                        let original_effective_dec = Decimal::from_u64(original_effective)
                            .ok_or_else(|| {
                                AuctionError::InternalError(format!(
                                    "Failed to convert effective qty {} to Decimal for order {:?}",
                                    original_effective, order_id
                                ))
                            })?;

                        let new_effective_qty_dec =
                            (original_effective_dec * reduction_factor).floor();

                        // Convert back to u64, handling potential errors (e.g., negative result, though unlikely)
                        let new_effective_qty_u64 = new_effective_qty_dec.to_u64()
                             .ok_or_else(|| AuctionError::InternalError(format!("Failed to convert pruned Decimal {} back to u64 for order {:?}", new_effective_qty_dec, order_id)))?;

                        // Apply the prune
                        order_to_prune.effective_quantity = new_effective_qty_u64;

                        // println!( // Debugging
                        //          "    Pruning Order {:?}: Original Effective={}, New Effective={}",
                        //          order_to_prune.id, original_effective, order_to_prune.effective_quantity);

                        // Also update the central map for consistency in the next loop
                        // This ensures find_clearing_for_resource sees the pruned quantity
                        if let Some(map_order) = order_map.get_mut(&order_to_prune.id) {
                            map_order.effective_quantity = order_to_prune.effective_quantity;
                        } else {
                            // Should not happen if current_orders and order_map are in sync
                            return Err(AuctionError::InternalError(format!(
                                "Order {:?} missing from map during pruning update",
                                order_id
                            )));
                        }
                    }
                    // else: Order might not be in current_orders if fully pruned earlier? Should be handled by effective_quantity check.
                }
            }
        }
    } // End of iteration loop

    // println!("--- Max Iterations Reached ---"); // Debugging
    // If loop finishes, max iterations were reached before convergence
    Err(AuctionError::MaxIterationsReached)
} // Result used here

// Helper to find clearing price and tentative fills for one resource
// Returns Result to propagate potential Decimal conversion errors
pub fn find_clearing_for_resource<'a>(
    orders: &[&'a Order],
    last_price: Option<Decimal>,
    order_map: &HashMap<OrderId, Order>, // Pass map ref
) -> Result<Option<ResourceClearing>, String> {
    // Return Result<Option<...>, ErrorString>

    // Filter and collect bids/asks - unchanged logic, but types use Decimal
    let bids: Vec<&Order> = orders
        .iter()
        .filter(|o| o.order_type == OrderType::Bid && o.effective_quantity > 0)
        .cloned()
        .collect();
    let mut asks: Vec<&Order> = orders
        .iter()
        .filter(|o| o.order_type == OrderType::Ask && o.effective_quantity > 0)
        .cloned()
        .collect();

    // Sort bids: Descending price, Ascending timestamp
    // Decimal implements Ord, so comparison is direct
    let mut sorted_bids = bids; // Create mutable copy for sorting
    sorted_bids.sort_unstable_by(|a, b| {
        b.limit_price
            .cmp(&a.limit_price) // Use cmp for Decimal
            .then_with(|| a.timestamp.cmp(&b.timestamp))
    });

    // Sort asks: Ascending price, Ascending timestamp
    asks.sort_unstable_by(|a, b| {
        a.limit_price
            .cmp(&b.limit_price) // Use cmp for Decimal
            .then_with(|| a.timestamp.cmp(&b.timestamp))
    });

    // --- Find Clearing Price ---
    // Collect potential prices (already Decimal)
    let mut potential_prices: Vec<Decimal> = sorted_bids
        .iter()
        .map(|o| o.limit_price)
        .chain(asks.iter().map(|o| o.limit_price))
        .collect();
    potential_prices.sort_unstable(); // sort is sufficient for Decimal
    potential_prices.dedup();

    let best_price; // Use Decimal Zero
    let mut max_volume = 0u64;
    let mut candidates = Vec::new(); // Store (price: Decimal, volume: u64) candidates

    for current_price in potential_prices.iter().rev() {
        // Iterate over prices
        // Calculate demand and supply at current_price
        let demand = sorted_bids
            .iter()
            .filter(|o| o.limit_price >= *current_price)
            .map(|o| o.effective_quantity)
            .sum::<u64>();
        let supply = asks
            .iter()
            .filter(|o| o.limit_price <= *current_price)
            .map(|o| o.effective_quantity)
            .sum::<u64>();
        let volume = demand.min(supply);

        if volume > 0 {
            if volume > max_volume {
                max_volume = volume;
                candidates.clear();
                candidates.push((current_price, volume));
            } else if volume == max_volume {
                candidates.push((current_price, volume));
            }
        }
    }

    if candidates.is_empty() {
        return Ok(None); // No trade possible
    }

    // --- Tie Breaking (using Decimal) ---
    if candidates.len() == 1 {
        best_price = *candidates[0].0;
    } else {
        if let Some(last_p) = last_price {
            // Sort by distance to last_p, then by price descending
            candidates.sort_unstable_by(|(p1, _), (p2, _)| {
                (**p1 - last_p)
                    .abs()
                    .cmp(&(**p2 - last_p).abs()) // Use abs() and cmp
                    .then_with(|| p2.cmp(p1)) // Secondary: highest price
            });
            best_price = *candidates[0].0;
        } else {
            // No last price, choose highest price among max volume candidates
            candidates.sort_unstable_by(|(p1, _), (p2, _)| p2.cmp(p1)); // Sort descending by price
            best_price = *candidates[0].0;
        }
    }

    let clearing_price = best_price;
    let matched_volume = max_volume;

    // --- Determine Tentative Fills based on Price-Time Priority ---
    let mut tentative_fills = Vec::new();
    // Use the already sorted lists
    let eligible_bids: Vec<&Order> = sorted_bids
        .into_iter() // consume sorted_bids
        .filter(|o| o.limit_price >= clearing_price)
        .collect();
    let eligible_asks: Vec<&Order> = asks
        .into_iter() // consume asks
        .filter(|o| o.limit_price <= clearing_price)
        .collect();

    let mut current_fills = HashMap::<OrderId, u64>::new(); // Track fills per order

    // Simplified matching loop (same logic as before, ensures volume limit)
    let mut bid_filled_volume = 0u64;
    for bid_order in &eligible_bids {
        if bid_filled_volume >= matched_volume {
            break;
        }
        let fill_amount = (matched_volume - bid_filled_volume).min(bid_order.effective_quantity);
        if fill_amount > 0 {
            *current_fills.entry(bid_order.id).or_insert(0) += fill_amount;
            bid_filled_volume += fill_amount;
        }
    }

    let mut ask_filled_volume = 0u64;
    for ask_order in &eligible_asks {
        if ask_filled_volume >= matched_volume {
            break;
        }
        let fill_amount = (matched_volume - ask_filled_volume).min(ask_order.effective_quantity);
        if fill_amount > 0 {
            // Check if this ask matches a filled bid before adding to fills map
            // (Prevents asks being added if only bids reached the volume limit first)
            // This check needs careful thought - the volume limit should inherently handle this.
            // The current approach fills bids up to volume, then asks up to volume,
            // and relies on the min(demand, supply) for the actual matched_volume.
            // Let's refine the fill logic slightly: Determine fillable quantity per order *before* adding to map.

            // Revisit fill logic: Iterate through eligible bids/asks simultaneously?
            // The current approach is simpler: check final map vs matched_volume
            *current_fills.entry(ask_order.id).or_insert(0) += fill_amount;
            ask_filled_volume += fill_amount;
        }
    }

    // Convert the fill map to the final list, ensuring total fills don't exceed matched_volume implicitly
    // (This assumes the logic above correctly calculated individual fills)
    // A check could be added here to sum fills and verify against matched_volume
    for (order_id, filled_quantity) in current_fills {
        if filled_quantity > 0 {
            // Ensure the order actually exists before adding fill
            if let Some(_order) = order_map.get(&order_id) {
                tentative_fills.push(TentativeFill {
                    order_id,
                    filled_quantity,
                });
                // Note: Since we process bids then asks, final_calculated_volume might exceed matched_volume *temporarily*
                // if we just sum naively. The *actual* matched volume is the constraint.
                // We rely on the individual min(remaining_volume, order_qty) calculations.
            } else {
                return Err(format!(
                    "Order {:?} not found in map during fill creation",
                    order_id
                ));
            }
        }
    }

    // Optional Sanity Check: Sum fills by side and compare? Might be overly complex here.
    // The matched_volume limit applied during the loops should suffice.

    Ok(Some(ResourceClearing {
        clearing_price,
        matched_volume,
        tentative_fills,
    }))
}

// --- Unit Tests (Updated for Decimal) ---
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec; // Import macro for tests
    use std::collections::HashMap;

    // Helper to create participants with Decimal currency
    pub fn create_participants(data: Vec<(u32, Decimal)>) -> HashMap<ParticipantId, Participant> {
        data.into_iter()
            .map(|(id_str, currency)| {
                let id = ParticipantId(id_str.to_string());
                (id.clone(), Participant { id, currency })
            })
            .collect()
    }

    // Helper to create orders with Decimal price
    pub fn create_order(
        id: usize,
        p_id: &str,
        r_id: &str,
        order_type: OrderType,
        qty: u64,
        price: Decimal, // <-- Use Decimal
        ts: u64,
    ) -> Order {
        Order {
            id: OrderId(id),
            participant_id: ParticipantId(p_id.to_string()),
            resource_id: ResourceId(r_id.to_string()),
            order_type,
            original_quantity: qty,
            effective_quantity: qty,
            limit_price: price,
            timestamp: ts,
        }
    }

    #[test]
    fn test_simple_match_sufficient_funds_decimal() {
        let orders = vec![
            create_order(1, "Alice", "CPU", OrderType::Ask, 10, dec!(100.0), 1), // Use dec!
            create_order(2, "Bob", "CPU", OrderType::Bid, 5, dec!(110.0), 2),    // Use dec!
        ];
        let participants =
            create_participants(vec![("Alice", dec!(1000.0)), ("Bob", dec!(1000.0))]); // Use dec!
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert_eq!(
                    success.clearing_prices[&ResourceId("CPU".to_string())],
                    dec!(110.0)
                ); // Expect Decimal
                assert_eq!(success.final_fills.len(), 2);

                let fill_alice = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(1))
                    .unwrap();
                let fill_bob = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(2))
                    .unwrap();

                assert_eq!(fill_alice.filled_quantity, 5);
                assert_eq!(fill_bob.filled_quantity, 5);
                assert_eq!(fill_alice.price, dec!(110.0)); // Expect Decimal
                assert_eq!(fill_bob.price, dec!(110.0));

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Alice".to_string()))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Bob".to_string()))
                    .unwrap();

                // Alice sells 5 @ 110 = +550 -> Final 1550.0
                // Bob buys 5 @ 110 = -550 -> Final 450.0
                assert_eq!(balance_alice.final_currency, dec!(1550.0)); // Direct comparison
                assert_eq!(balance_bob.final_currency, dec!(450.0));
            }
            Err(e) => {
                panic!("Auction should have succeeded, failed with {:?}", e)
            }
        }
    }

    #[test]
    fn test_no_match_price_gap_decimal() {
        let orders = vec![
            create_order(1, "Alice", "CPU", OrderType::Ask, 10, dec!(110.0), 1),
            create_order(2, "Bob", "CPU", OrderType::Bid, 5, dec!(100.0), 2),
        ];
        let participants =
            create_participants(vec![("Alice", dec!(1000.0)), ("Bob", dec!(1000.0))]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert!(
                    success
                        .clearing_prices
                        .get(&ResourceId("CPU".to_string()))
                        .is_none()
                );
                assert!(success.final_fills.is_empty());
                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Alice".to_string()))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Bob".to_string()))
                    .unwrap();
                assert_eq!(balance_alice.final_currency, dec!(1000.0)); // Balances unchanged
                assert_eq!(balance_bob.final_currency, dec!(1000.0));
            }
            Err(e) => panic!(
                "Auction should have succeeded (with no trades), failed with {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_budget_constraint_pruning_decimal() {
        let orders = vec![
            create_order(1, "Alice", "CPU", OrderType::Ask, 10, dec!(100.0), 1),
            create_order(2, "Bob", "CPU", OrderType::Bid, 8, dec!(110.0), 2),
            create_order(3, "Carol", "RAM", OrderType::Ask, 5, dec!(50.0), 3),
            create_order(4, "Bob", "RAM", OrderType::Bid, 4, dec!(60.0), 4),
        ];
        let participants = create_participants(vec![
            ("Alice", dec!(1000.0)),
            ("Bob", dec!(700.0)), // Bob's budget
            ("Carol", dec!(1000.0)),
        ]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                // Prices clear high due to tie-breaking
                assert_eq!(
                    success.clearing_prices[&ResourceId("CPU".to_string())],
                    dec!(110.0)
                );
                assert_eq!(
                    success.clearing_prices[&ResourceId("RAM".to_string())],
                    dec!(60.0)
                );
                assert_eq!(success.final_fills.len(), 4);

                // Final state after pruning (as determined before):
                // Bob CPU Bid Qty = 5
                // Bob RAM Bid Qty = 2

                let fill_bob_cpu = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(2))
                    .unwrap();
                let fill_bob_ram = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(4))
                    .unwrap();
                let fill_alice_cpu = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(1))
                    .unwrap();
                let fill_carol_ram = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(3))
                    .unwrap();

                assert_eq!(fill_bob_cpu.filled_quantity, 5);
                assert_eq!(fill_bob_ram.filled_quantity, 2);
                assert_eq!(fill_alice_cpu.filled_quantity, 5);
                assert_eq!(fill_carol_ram.filled_quantity, 2);

                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Bob".to_string()))
                    .unwrap();
                // Bob bought 5 CPU @ 110 (cost 550) + 2 RAM @ 60 (cost 120) = Total cost 670
                // Final balance = 700 - 670 = 30
                assert_eq!(balance_bob.final_currency, dec!(30.0));

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Alice".to_string()))
                    .unwrap();
                // Alice sold 5 CPU @ 110 (proceeds 550) -> Final 1550.0
                assert_eq!(balance_alice.final_currency, dec!(1550.0));

                let balance_carol = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Carol".to_string()))
                    .unwrap();
                // Carol sold 2 RAM @ 60 (proceeds 120) -> Final 1120.0
                assert_eq!(balance_carol.final_currency, dec!(1120.0));
            }
            Err(e) => panic!(
                "Auction should have succeeded after pruning, failed with {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_price_time_priority_decimal() {
        let orders = vec![
            create_order(1, "Alice", "GPU", OrderType::Ask, 5, dec!(500.0), 10),
            create_order(2, "Bob", "GPU", OrderType::Bid, 3, dec!(510.0), 5),
            create_order(3, "Carol", "GPU", OrderType::Bid, 4, dec!(500.0), 8),
            create_order(4, "David", "GPU", OrderType::Bid, 2, dec!(500.0), 12),
        ];
        let participants = create_participants(vec![
            ("Alice", dec!(10000.0)),
            ("Bob", dec!(10000.0)),
            ("Carol", dec!(10000.0)),
            ("David", dec!(10000.0)),
        ]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert_eq!(
                    success.clearing_prices[&ResourceId("GPU".to_string())],
                    dec!(500.0)
                );
                assert_eq!(success.final_fills.len(), 3);

                let fill_alice = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(1))
                    .unwrap();
                let fill_bob = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(2));
                let fill_carol = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(3));
                let fill_david = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(4));

                assert_eq!(fill_alice.filled_quantity, 5);
                assert!(fill_bob.is_some());
                assert_eq!(fill_bob.unwrap().filled_quantity, 3);
                assert!(fill_carol.is_some());
                assert_eq!(fill_carol.unwrap().filled_quantity, 2);
                assert!(fill_david.is_none());
            }
            Err(e) => {
                panic!("Auction should have succeeded, failed with {:?}", e)
            }
        }
    }

    #[test]
    fn test_max_iterations_failure_decimal() {
        // Scenario that previously converged in 1 iter
        let orders = vec![
            create_order(1, "Alice", "A", OrderType::Ask, 10, dec!(10.0), 1),
            create_order(2, "Bob", "A", OrderType::Bid, 5, dec!(11.0), 2),
            create_order(3, "Alice", "B", OrderType::Bid, 6, dec!(10.0), 3),
            create_order(4, "Bob", "B", OrderType::Ask, 8, dec!(9.0), 4),
        ];
        let participants = create_participants(vec![("Alice", dec!(55.0)), ("Bob", dec!(45.0))]);

        // Run with max_iterations = 0
        let result_iter_0 = run_auction(orders, participants, 0, HashMap::new());
        match result_iter_0 {
            Ok(_) => {
                panic!("Auction should have failed with max_iterations = 0")
            }
            Err(e) => {
                assert!(matches!(e, AuctionError::MaxIterationsReached));
            }
        }
    }

    // --- Barter Tests (Updated for Decimal) ---

    #[test]
    fn test_barter_simple_direct_decimal() {
        let orders = vec![
            create_order(1, "Alice", "X", OrderType::Ask, 1, dec!(100.0), 1),
            create_order(2, "Bob", "X", OrderType::Bid, 1, dec!(100.0), 2),
            create_order(3, "Alice", "Y", OrderType::Bid, 1, dec!(100.0), 3),
            create_order(4, "Bob", "Y", OrderType::Ask, 1, dec!(100.0), 4),
        ];
        let participants = create_participants(vec![("Alice", dec!(0.0)), ("Bob", dec!(0.0))]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert_eq!(
                    success.clearing_prices[&ResourceId("X".to_string())],
                    dec!(100.0)
                );
                assert_eq!(
                    success.clearing_prices[&ResourceId("Y".to_string())],
                    dec!(100.0)
                );
                assert_eq!(success.final_fills.len(), 4);

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Alice".to_string()))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Bob".to_string()))
                    .unwrap();

                assert_eq!(balance_alice.final_currency, dec!(0.0));
                assert_eq!(balance_bob.final_currency, dec!(0.0));
            }
            Err(e) => {
                panic!("Barter auction should have succeeded, failed with {:?}", e)
            }
        }
    }

    #[test]
    fn test_barter_exact_fraction_decimal() {
        // Alice sells 1 X @ 10.0, buys 3 Y @ 10/3
        // Bob buys 1 X @ 10.0, sells 3 Y @ 10/3
        let _price_y = dec!(10.0) / dec!(3.0); // Decimal handles this exactly if possible within precision
        // println!("Price Y: {}", price_y); // Note: Default precision might round this

        // Let's use a price representable exactly: sell 1 X @ 10.50, buy 3 Y @ 3.50
        let price_y_exact = dec!(3.50);
        let orders = vec![
            create_order(1, "Alice", "X", OrderType::Ask, 1, dec!(10.50), 1),
            create_order(2, "Bob", "X", OrderType::Bid, 1, dec!(10.50), 2),
            create_order(3, "Alice", "Y", OrderType::Bid, 3, price_y_exact, 3), // 3 * 3.50 = 10.50
            create_order(4, "Bob", "Y", OrderType::Ask, 3, price_y_exact, 4),
        ];
        let participants = create_participants(vec![("Alice", dec!(0.0)), ("Bob", dec!(0.0))]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert_eq!(
                    success.clearing_prices[&ResourceId("X".to_string())],
                    dec!(10.50)
                );
                assert_eq!(
                    success.clearing_prices[&ResourceId("Y".to_string())],
                    price_y_exact
                );
                assert_eq!(success.final_fills.len(), 4);

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Alice".to_string()))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId("Bob".to_string()))
                    .unwrap();

                // Alice Cost = 3 * 3.50 = 10.50. Proceeds = 1 * 10.50 = 10.50. Net = 0.
                assert_eq!(
                    balance_alice.final_currency,
                    dec!(0.0),
                    "Alice final balance {} not zero",
                    balance_alice.final_currency
                );
                assert_eq!(
                    balance_bob.final_currency,
                    dec!(0.0),
                    "Bob final balance {} not zero",
                    balance_bob.final_currency
                );
            }
            Err(e) => panic!(
                "Barter auction with exact Decimal fractions failed: {:?}",
                e
            ),
        }
    }

    // Other barter tests (partial fills, three-way) should also work correctly with Decimal
    // without needing f64 tolerance checks.
} // end tests mod