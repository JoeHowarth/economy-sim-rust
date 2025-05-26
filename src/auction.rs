//! # Double Auction Module
//!
//! This module implements a multi-resource double auction mechanism with budget constraint enforcement.
//! 
//! ## Overview
//! 
//! The auction operates as an iterative process that finds market-clearing prices where supply meets demand
//! while ensuring no participant spends more than their available currency. This is critical for trading
//! systems where participants have limited budgets.
//!
//! ## Key Features
//!
//! - **Multi-resource clearing**: Simultaneously clears multiple resource markets (e.g., wood and food)
//! - **Budget enforcement**: Prevents participants from overspending through iterative order pruning
//! - **Price discovery**: Finds equilibrium prices that maximize trading volume
//! - **Decimal precision**: Uses rust_decimal for exact financial calculations without floating-point errors
//!
//! ## Algorithm
//!
//! 1. **Initial matching**: For each resource, find the price that maximizes matched volume
//! 2. **Budget check**: Calculate net cash flows for each participant
//! 3. **Order pruning**: If any participant would overspend, reduce their buy orders proportionally
//! 4. **Iterate**: Repeat until no participant exceeds their budget or max iterations reached
//!
//! ## Why This Approach?
//!
//! Traditional double auctions can lead to participants owing more than they have. Our iterative
//! pruning algorithm ensures feasibility while maintaining fairness - orders are reduced proportionally
//! based on how much a participant is over-budget, preserving their relative preferences.

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

// Result of net outflow calculations
struct NetOutflowResults {
    gross_outflows: HashMap<ParticipantId, Decimal>,
    net_outflows: HashMap<ParticipantId, Decimal>,
    buyer_fills: HashMap<ParticipantId, Vec<(OrderId, u64, Decimal)>>,
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

/// Runs a multi-resource double auction with budget constraint enforcement.
///
/// This is the main entry point for the auction system. It takes orders from multiple participants
/// for multiple resources and finds market-clearing prices while ensuring no participant exceeds
/// their available currency.
///
/// # Arguments
///
/// * `orders` - All buy and sell orders to be matched
/// * `participants` - Map of participant IDs to their available currency
/// * `max_iterations` - Maximum pruning iterations before giving up (prevents infinite loops)
/// * `last_clearing_prices` - Previous clearing prices for tie-breaking (improves price stability)
///
/// # Returns
///
/// * `Ok(AuctionSuccess)` - Contains final fills, participant balances, and clearing prices
/// * `Err(AuctionError)` - If max iterations reached or internal error occurs
///
/// # Algorithm Details
///
/// The function iteratively:
/// 1. Groups orders by resource and finds tentative clearing prices
/// 2. Calculates net cash outflows for each participant
/// 3. Identifies participants who would exceed their budget
/// 4. Proportionally reduces (prunes) over-budget participants' buy orders
/// 5. Repeats until all budget constraints are satisfied
///
/// This ensures a feasible outcome where all trades can actually be settled.
/// Groups orders by resource ID, filtering out orders with zero effective quantity.
fn group_orders_by_resource(orders: &[Order]) -> HashMap<ResourceId, Vec<&Order>> {
    let mut resource_orders: HashMap<ResourceId, Vec<&Order>> = HashMap::new();
    
    for order in orders.iter() {
        if order.effective_quantity > 0 {
            resource_orders
                .entry(order.resource_id.clone())
                .or_default()
                .push(order);
        }
    }
    
    resource_orders
}

/// Calculates net cash outflows for each participant based on clearing results.
/// Returns (net_outflows, costs, tentative_buy_fills_info)
fn calculate_net_outflows(
    iteration_clearings: &HashMap<ResourceId, ResourceClearing>,
    order_map: &HashMap<OrderId, Order>,
) -> Result<NetOutflowResults, AuctionError> {
    let mut net_outflows: HashMap<ParticipantId, Decimal> = HashMap::new();
    let mut costs: HashMap<ParticipantId, Decimal> = HashMap::new();
    // Store only needed info for pruning: (OrderID, FilledQty, ClearingPrice)
    let mut tentative_buy_fills_info: HashMap<ParticipantId, Vec<(OrderId, u64, Decimal)>> =
        HashMap::new();

    for clearing in iteration_clearings.values() {
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
    
    Ok(NetOutflowResults {
        gross_outflows: costs,
        net_outflows,
        buyer_fills: tentative_buy_fills_info,
    })
}

/// Applies budget pruning to orders for participants who are short on funds.
/// Proportionally reduces buy orders to ensure budget constraints are met.
fn apply_budget_pruning(
    short_participants_info: &[(ParticipantId, Decimal)],
    costs: &HashMap<ParticipantId, Decimal>,
    tentative_buy_fills_info: &HashMap<ParticipantId, Vec<(OrderId, u64, Decimal)>>,
    current_orders: &mut [Order],
    order_map: &mut HashMap<OrderId, Order>,
) -> Result<(), AuctionError> {
    for (participant_id, shortfall) in short_participants_info {
        let total_cost = costs.get(participant_id).copied().unwrap_or(Decimal::ZERO);

        // Avoid division by zero or pruning if no cost basis
        if total_cost <= Decimal::ZERO {
            continue;
        }
        // Ensure shortfall isn't somehow negative (shouldn't happen)
        if *shortfall <= Decimal::ZERO {
            continue;
        }

        // Calculate reduction percentage. Ensure it's capped at 1.0 (100%)
        // Example: If participant needs 1000 but only has 700, shortfall = 300
        // If total buy cost = 1000, reduction = 30%, so scale all buys by 70%
        let reduction_percentage = (*shortfall / total_cost).min(Decimal::ONE);
        let reduction_factor = Decimal::ONE - reduction_percentage; // Factor to multiply quantities by

        // println!( // Debugging
        //     "  Pruning Participant {:?}: Shortfall={}, Cost={}, Reduction%={:.2}",
        //     participant_id, shortfall, total_cost, reduction_percentage * dec!(100.0)
        // );

        // Use the collected buy fill info
        if let Some(buy_fills) = tentative_buy_fills_info.get(participant_id) {
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
    
    Ok(())
}

/// Creates the final auction results after convergence is reached.
fn create_final_results(
    iteration_clearings: HashMap<ResourceId, ResourceClearing>,
    net_outflows: HashMap<ParticipantId, Decimal>,
    current_participants: &mut HashMap<ParticipantId, Participant>,
    order_map: &HashMap<OrderId, Order>,
) -> Result<AuctionSuccess, AuctionError> {
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
                price, // Already a Decimal
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

    Ok(AuctionSuccess {
        final_fills,
        final_balances,
        clearing_prices: final_clearing_prices,
    })
}

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

        // 1. Group orders by resource (using current effective quantities)
        // This separates the multi-resource problem into independent single-resource auctions
        let resource_orders = group_orders_by_resource(&current_orders);

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
        // Net outflow = total cost of buys - total proceeds from sells
        // Positive outflow means participant needs to pay money
        let outflow_results = calculate_net_outflows(&iteration_clearings, &order_map)?;
        let net_outflows = outflow_results.net_outflows;
        let costs = outflow_results.gross_outflows;
        let tentative_buy_fills_info = outflow_results.buyer_fills;

        // 5. Identify and Prune Short Participants
        // A participant is "short" if their net outflow exceeds available currency
        // Shortfall = how much they're over budget
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
            return create_final_results(
                iteration_clearings,
                net_outflows,
                &mut current_participants,
                &order_map,
            );
        }

        // --- Pruning Logic ---
        // This is the key innovation: proportionally reduce buy orders to fit budget
        // Reduction percentage = shortfall / total_cost_of_buys
        // This maintains relative preferences while ensuring feasibility
        apply_budget_pruning(
            &short_participants_info,
            &costs,
            &tentative_buy_fills_info,
            &mut current_orders,
            &mut order_map,
        )?;
    } // End of iteration loop

    // println!("--- Max Iterations Reached ---"); // Debugging
    // If loop finishes, max iterations were reached before convergence
    Err(AuctionError::MaxIterationsReached)
} // Result used here

/// Collects eligible orders and sorts them by price and timestamp priority.
/// Returns (sorted_bids, sorted_asks)
fn collect_eligible_orders<'a>(orders: &[&'a Order]) -> (Vec<&'a Order>, Vec<&'a Order>) {
    // Filter and collect bids/asks
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
    let mut sorted_bids = bids;
    sorted_bids.sort_unstable_by(|a, b| {
        b.limit_price
            .cmp(&a.limit_price)
            .then_with(|| a.timestamp.cmp(&b.timestamp))
    });

    // Sort asks: Ascending price, Ascending timestamp
    asks.sort_unstable_by(|a, b| {
        a.limit_price
            .cmp(&b.limit_price)
            .then_with(|| a.timestamp.cmp(&b.timestamp))
    });

    (sorted_bids, asks)
}

/// Finds the best clearing price that maximizes trading volume.
/// Returns Some((price, volume)) or None if no trades are possible.
fn find_best_clearing(
    sorted_bids: &[&Order],
    asks: &[&Order],
    last_price: Option<Decimal>,
) -> Result<Option<(Decimal, u64)>, String> {
    // We test every unique limit price from all orders as a potential clearing price
    // This guarantees we find the optimal price (no need for binary search)
    let mut potential_prices: Vec<Decimal> = sorted_bids
        .iter()
        .map(|o| o.limit_price)
        .chain(asks.iter().map(|o| o.limit_price))
        .collect();
    potential_prices.sort_unstable();
    potential_prices.dedup();

    let mut max_volume = 0u64;
    let mut candidates = Vec::new(); // Store (price: Decimal, volume: u64) candidates

    for current_price in potential_prices.iter().rev() {
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
            match volume.cmp(&max_volume) {
                std::cmp::Ordering::Greater => {
                    max_volume = volume;
                    candidates.clear();
                    candidates.push((current_price, volume));
                }
                std::cmp::Ordering::Equal => {
                    candidates.push((current_price, volume));
                }
                std::cmp::Ordering::Less => {}
            }
        }
    }

    if candidates.is_empty() {
        return Ok(None); // No trade possible
    }

    // Tie Breaking
    // When multiple prices yield same max volume, we need consistent tie-breaking
    // Using last price improves stability; without it, we favor sellers (highest price)
    let best_price = if candidates.len() == 1 {
        *candidates[0].0
    } else if let Some(last_p) = last_price {
        // Sort by distance to last_p, then by price descending
        candidates.sort_unstable_by(|(p1, _), (p2, _)| {
            (**p1 - last_p)
                .abs()
                .cmp(&(**p2 - last_p).abs())
                .then_with(|| p2.cmp(p1)) // Secondary: highest price
        });
        *candidates[0].0
    } else {
        // No last price, choose highest price among max volume candidates
        candidates.sort_unstable_by(|(p1, _), (p2, _)| p2.cmp(p1));
        *candidates[0].0
    };

    Ok(Some((best_price, max_volume)))
}

/// Creates tentative fills for orders based on price-time priority.
fn create_tentative_fills(
    sorted_bids: Vec<&Order>,
    asks: Vec<&Order>,
    clearing_price: Decimal,
    matched_volume: u64,
    order_map: &HashMap<OrderId, Order>,
) -> Result<Vec<TentativeFill>, String> {
    let mut tentative_fills = Vec::new();
    
    // Filter to eligible orders
    let eligible_bids: Vec<&Order> = sorted_bids
        .into_iter()
        .filter(|o| o.limit_price >= clearing_price)
        .collect();
    let eligible_asks: Vec<&Order> = asks
        .into_iter()
        .filter(|o| o.limit_price <= clearing_price)
        .collect();

    let mut current_fills = HashMap::<OrderId, u64>::new();

    // Fill bids up to matched volume
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

    // Fill asks up to matched volume
    let mut ask_filled_volume = 0u64;
    for ask_order in &eligible_asks {
        if ask_filled_volume >= matched_volume {
            break;
        }
        let fill_amount = (matched_volume - ask_filled_volume).min(ask_order.effective_quantity);
        if fill_amount > 0 {
            *current_fills.entry(ask_order.id).or_insert(0) += fill_amount;
            ask_filled_volume += fill_amount;
        }
    }

    // Convert fill map to tentative fills
    for (order_id, filled_quantity) in current_fills {
        if filled_quantity > 0 {
            // Ensure the order actually exists before adding fill
            if let Some(_order) = order_map.get(&order_id) {
                tentative_fills.push(TentativeFill {
                    order_id,
                    filled_quantity,
                });
            } else {
                return Err(format!(
                    "Order {:?} not found in map during fill creation",
                    order_id
                ));
            }
        }
    }

    Ok(tentative_fills)
}

/// Finds the market-clearing price and matched orders for a single resource.
///
/// This function implements the core price discovery mechanism for one resource type.
/// It finds the price that maximizes trading volume while respecting order limit prices.
///
/// # Arguments
///
/// * `orders` - All orders for this specific resource (both bids and asks)
/// * `last_price` - Previous clearing price for tie-breaking
/// * `order_map` - Complete order lookup map for fill tracking
///
/// # Returns
///
/// * `Ok(Some(ResourceClearing))` - Clearing price and tentative fills if trades possible
/// * `Ok(None)` - If no trades possible (no price overlap between bids and asks)
/// * `Err(String)` - If internal error occurs
///
/// # Price Discovery Process
///
/// 1. **Sort orders**: Bids by descending price, asks by ascending price
/// 2. **Find candidate prices**: Test each unique limit price from all orders
/// 3. **Calculate volume**: At each price, volume = min(demand, supply)
/// 4. **Select best price**: Choose price with maximum volume
/// 5. **Tie-breaking**: If multiple prices have same volume:
///    - With last_price: Choose closest to last price
///    - Without last_price: Choose highest price (favors sellers)
///
/// # Fill Allocation
///
/// Once clearing price is found, orders are filled using price-time priority:
/// - Orders with better prices filled first
/// - Among same price, earlier orders (lower timestamp) filled first
/// - Partial fills allowed to match exact volume
pub fn find_clearing_for_resource(
    orders: &[&Order],
    last_price: Option<Decimal>,
    order_map: &HashMap<OrderId, Order>, // Pass map ref
) -> Result<Option<ResourceClearing>, String> {
    // Return Result<Option<...>, ErrorString>

    // Collect and sort eligible orders
    let (sorted_bids, asks) = collect_eligible_orders(orders);

    // Find the best clearing price and volume
    let clearing_result = find_best_clearing(&sorted_bids, &asks, last_price)?;
    
    let (clearing_price, matched_volume) = match clearing_result {
        Some((price, volume)) => (price, volume),
        None => return Ok(None), // No trade possible
    };

    // Create tentative fills based on price-time priority
    let tentative_fills = create_tentative_fills(
        sorted_bids,
        asks,
        clearing_price,
        matched_volume,
        order_map,
    )?;

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

    // Constants for participant IDs
    const ALICE: u32 = 1;
    const BOB: u32 = 2;
    const CAROL: u32 = 3;
    const DAVID: u32 = 4;

    // Helper to create participants with Decimal currency
    pub fn create_participants(data: Vec<(u32, Decimal)>) -> HashMap<ParticipantId, Participant> {
        data.into_iter()
            .map(|(id_num, currency)| {
                let id = ParticipantId(id_num);
                (id.clone(), Participant { id, currency })
            })
            .collect()
    }

    // Helper to create orders with Decimal price
    pub fn create_order(
        id: usize,
        p_id: u32,
        r_id: &str,
        order_type: OrderType,
        qty: u64,
        price: Decimal, // <-- Use Decimal
        ts: u64,
    ) -> Order {
        Order {
            id: OrderId(id),
            participant_id: ParticipantId(p_id),
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
            create_order(1, ALICE, "CPU", OrderType::Ask, 10, dec!(100.0), 1),
            create_order(2, BOB, "CPU", OrderType::Bid, 5, dec!(110.0), 2),
        ];
        let participants = create_participants(vec![(ALICE, dec!(1000.0)), (BOB, dec!(1000.0))]);
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
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(BOB))
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
            create_order(1, ALICE, "CPU", OrderType::Ask, 10, dec!(110.0), 1),
            create_order(2, BOB, "CPU", OrderType::Bid, 5, dec!(100.0), 2),
        ];
        let participants = create_participants(vec![(ALICE, dec!(1000.0)), (BOB, dec!(1000.0))]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                assert!(
                    !success
                        .clearing_prices
                        .contains_key(&ResourceId("CPU".to_string()))
                );
                assert!(success.final_fills.is_empty());
                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(BOB))
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
            create_order(1, ALICE, "CPU", OrderType::Ask, 10, dec!(100.0), 1),
            create_order(2, BOB, "CPU", OrderType::Bid, 8, dec!(110.0), 2),
            create_order(3, CAROL, "RAM", OrderType::Ask, 5, dec!(50.0), 3),
            create_order(4, BOB, "RAM", OrderType::Bid, 4, dec!(60.0), 4),
        ];
        let participants = create_participants(vec![
            (ALICE, dec!(1000.0)),
            (BOB, dec!(700.0)), // Bob's budget
            (CAROL, dec!(1000.0)),
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
                    .find(|b| b.participant_id == ParticipantId(BOB))
                    .unwrap();
                // Bob bought 5 CPU @ 110 (cost 550) + 2 RAM @ 60 (cost 120) = Total cost 670
                // Final balance = 700 - 670 = 30
                assert_eq!(balance_bob.final_currency, dec!(30.0));

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                // Alice sold 5 CPU @ 110 (proceeds 550) -> Final 1550.0
                assert_eq!(balance_alice.final_currency, dec!(1550.0));

                let balance_carol = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(CAROL))
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
            create_order(1, ALICE, "GPU", OrderType::Ask, 5, dec!(500.0), 10),
            create_order(2, BOB, "GPU", OrderType::Bid, 3, dec!(510.0), 5),
            create_order(3, CAROL, "GPU", OrderType::Bid, 4, dec!(500.0), 8),
            create_order(4, DAVID, "GPU", OrderType::Bid, 2, dec!(500.0), 12),
        ];
        let participants = create_participants(vec![
            (ALICE, dec!(10000.0)),
            (BOB, dec!(10000.0)),
            (CAROL, dec!(10000.0)),
            (DAVID, dec!(10000.0)),
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
            create_order(1, ALICE, "A", OrderType::Ask, 10, dec!(10.0), 1),
            create_order(2, BOB, "A", OrderType::Bid, 5, dec!(11.0), 2),
            create_order(3, ALICE, "B", OrderType::Bid, 6, dec!(10.0), 3),
            create_order(4, BOB, "B", OrderType::Ask, 8, dec!(9.0), 4),
        ];
        let participants = create_participants(vec![(ALICE, dec!(55.0)), (BOB, dec!(45.0))]);

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
            create_order(1, ALICE, "X", OrderType::Ask, 1, dec!(100.0), 1),
            create_order(2, BOB, "X", OrderType::Bid, 1, dec!(100.0), 2),
            create_order(3, ALICE, "Y", OrderType::Bid, 1, dec!(100.0), 3),
            create_order(4, BOB, "Y", OrderType::Ask, 1, dec!(100.0), 4),
        ];
        let participants = create_participants(vec![(ALICE, dec!(0.0)), (BOB, dec!(0.0))]);
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
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(BOB))
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
            create_order(1, ALICE, "X", OrderType::Ask, 1, dec!(10.50), 1),
            create_order(2, BOB, "X", OrderType::Bid, 1, dec!(10.50), 2),
            create_order(3, ALICE, "Y", OrderType::Bid, 3, price_y_exact, 3), // 3 * 3.50 = 10.50
            create_order(4, BOB, "Y", OrderType::Ask, 3, price_y_exact, 4),
        ];
        let participants = create_participants(vec![(ALICE, dec!(0.0)), (BOB, dec!(0.0))]);
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
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(BOB))
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

    #[test]
    fn test_multi_resource_clearing() {
        // Test that auction can handle multiple resources independently
        let orders = vec![
            // Wood market
            create_order(1, ALICE, "wood", OrderType::Bid, 10, dec!(15.0), 1),
            create_order(2, BOB, "wood", OrderType::Ask, 10, dec!(12.0), 2),
            // Food market
            create_order(3, CAROL, "food", OrderType::Bid, 5, dec!(20.0), 3),
            create_order(4, DAVID, "food", OrderType::Ask, 5, dec!(18.0), 4),
        ];
        let participants = create_participants(vec![
            (ALICE, dec!(200.0)),
            (BOB, dec!(200.0)),
            (CAROL, dec!(200.0)),
            (DAVID, dec!(200.0)),
        ]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                // Should have clearing results for both resources
                assert_eq!(success.clearing_prices.len(), 2);

                // Check wood market cleared - auction chooses from submitted prices
                let wood_price = success.clearing_prices[&ResourceId("wood".to_string())];
                assert!(wood_price == dec!(12.0) || wood_price == dec!(15.0)); // Should be one of the submitted prices

                // Check food market cleared - auction chooses from submitted prices
                let food_price = success.clearing_prices[&ResourceId("food".to_string())];
                assert!(food_price == dec!(18.0) || food_price == dec!(20.0)); // Should be one of the submitted prices

                // Should have fills for all 4 orders
                assert_eq!(success.final_fills.len(), 4);

                // Verify quantities
                let wood_bid_fill = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(1))
                    .unwrap();
                assert_eq!(wood_bid_fill.filled_quantity, 10);

                let food_bid_fill = success
                    .final_fills
                    .iter()
                    .find(|f| f.order_id == OrderId(3))
                    .unwrap();
                assert_eq!(food_bid_fill.filled_quantity, 5);
            }
            Err(e) => panic!(
                "Multi-resource auction should have succeeded, failed with {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_resource_isolation() {
        // Test that orders for different resources don't interfere
        let orders = vec![
            // High wood bid
            create_order(1, ALICE, "wood", OrderType::Bid, 5, dec!(50.0), 1),
            // Low food ask - should not match with wood bid
            create_order(2, BOB, "food", OrderType::Ask, 5, dec!(10.0), 2),
        ];
        let participants = create_participants(vec![(ALICE, dec!(500.0)), (BOB, dec!(500.0))]);
        let result = run_auction(orders, participants, 5, HashMap::new());

        match result {
            Ok(success) => {
                // Should have no fills since resources don't match
                assert_eq!(success.final_fills.len(), 0);
                assert_eq!(success.clearing_prices.len(), 0);

                // Balances should be unchanged
                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                let balance_bob = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(BOB))
                    .unwrap();
                assert_eq!(balance_alice.final_currency, dec!(500.0));
                assert_eq!(balance_bob.final_currency, dec!(500.0));
            }
            Err(e) => panic!(
                "Resource isolation test should have succeeded, failed with {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_multi_resource_budget_constraint() {
        // Test budget constraints across multiple resources
        let orders = vec![
            // Alice wants to buy both wood and food
            create_order(1, ALICE, "wood", OrderType::Bid, 10, dec!(20.0), 1),
            create_order(2, ALICE, "food", OrderType::Bid, 10, dec!(30.0), 2),
            // Sellers
            create_order(3, BOB, "wood", OrderType::Ask, 10, dec!(20.0), 3),
            create_order(4, CAROL, "food", OrderType::Ask, 10, dec!(30.0), 4),
        ];
        // Alice only has 400, needs 500 for both orders
        let participants = create_participants(vec![
            (ALICE, dec!(400.0)),
            (BOB, dec!(1000.0)),
            (CAROL, dec!(1000.0)),
        ]);
        let result = run_auction(orders, participants, 10, HashMap::new());

        match result {
            Ok(success) => {
                // Alice should have had orders pruned
                let alice_fills: Vec<_> = success
                    .final_fills
                    .iter()
                    .filter(|f| f.participant_id == ParticipantId(ALICE))
                    .collect();

                let total_cost: Decimal = alice_fills
                    .iter()
                    .map(|f| Decimal::from(f.filled_quantity) * f.price)
                    .sum();

                // Total cost should not exceed Alice's budget
                assert!(total_cost <= dec!(400.0));

                let balance_alice = success
                    .final_balances
                    .iter()
                    .find(|b| b.participant_id == ParticipantId(ALICE))
                    .unwrap();
                assert!(balance_alice.final_currency >= dec!(0.0));
            }
            Err(e) => panic!("Multi-resource budget constraint test failed: {:?}", e),
        }
    }
} // end tests mod
