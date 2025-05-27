//! Strategy system for village decision-making in the economic simulation.
//!
//! This module provides different AI strategies that villages can use to allocate workers
//! and make trading decisions. Each strategy represents a different approach to resource
//! management and growth, from conservative survival to aggressive trading.
//!
//! # Strategy Types
//!
//! - **Default**: Fixed allocation (70% wood, 20% food, 10% construction), no trading
//! - **Survival**: Prioritizes immediate needs with resource buffers
//! - **Growth**: Focuses on population expansion through housing
//! - **Trading**: Specializes in one resource and trades aggressively
//! - **Balanced**: Adapts dynamically to current needs
//! - **Greedy**: Maximizes immediate production value

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

use crate::scenario::StrategyConfig;

// === HELPER FUNCTIONS ===

/// Calculate how many days a resource will last given current consumption rate
fn calculate_resource_days(amount: Decimal, consumption_per_day: Decimal) -> u32 {
    if consumption_per_day > dec!(0) {
        (amount / consumption_per_day).to_u32().unwrap_or(0)
    } else {
        999
    }
}

/// Calculate urgency score for a resource (0.0 to 1.0)
/// Higher urgency means lower days of supply
fn calculate_resource_urgency(days_of_supply: u32, half_life_days: f64) -> f64 {
    1.0 / (1.0 + days_of_supply as f64 / half_life_days)
}

/// Calculate bid price for food based on market price and urgency
fn calculate_food_bid_price(market_price: Option<Decimal>, multiplier: Decimal) -> Decimal {
    market_price.unwrap_or(get_default_price(false)) * multiplier
}

/// Calculate bid price for wood based on market price and urgency
fn calculate_wood_bid_price(market_price: Option<Decimal>, multiplier: Decimal) -> Decimal {
    market_price.unwrap_or(get_default_price(true)) * multiplier
}

/// Calculate ask price for food based on market price and discount
fn calculate_food_ask_price(market_price: Option<Decimal>, multiplier: Decimal) -> Decimal {
    market_price.unwrap_or(get_default_price(false)) * multiplier
}

/// Calculate ask price for wood based on market price and discount
fn calculate_wood_ask_price(market_price: Option<Decimal>, multiplier: Decimal) -> Decimal {
    market_price.unwrap_or(get_default_price(true)) * multiplier
}

/// Get default price for a resource type
fn get_default_price(is_wood: bool) -> Decimal {
    if is_wood { dec!(5.0) } else { dec!(1.0) }
}

/// Calculate marginal productivity for a resource given current workers
/// Returns productivity of the next worker assigned
fn calculate_marginal_productivity(current_workers: u32, slots: (u32, u32)) -> Decimal {
    if current_workers < slots.0 {
        // First slot: 100% productivity
        dec!(1.0)
    } else if current_workers < slots.0 + slots.1 {
        // Second slot: 75% productivity
        dec!(0.75)
    } else {
        // Beyond slots: 0% productivity
        dec!(0.0)
    }
}

/// Calculate marginal cost of producing one unit of a resource
/// Cost = 1 / (productivity * production_rate)
fn calculate_marginal_cost(
    current_workers: u32, 
    slots: (u32, u32),
    base_production_rate: Decimal
) -> Decimal {
    let productivity = calculate_marginal_productivity(current_workers, slots);
    if productivity > dec!(0) {
        dec!(1) / (productivity * base_production_rate)
    } else {
        dec!(1000000) // Infinite cost if no productivity
    }
}

/// Check if village can afford a quantity at a given price
fn can_afford_quantity(
    money: Decimal,
    price: Decimal,
    quantity: u32,
    reserve_fraction: Decimal,
) -> bool {
    let total_cost = price * Decimal::from(quantity);
    let available_money = money * (dec!(1) - reserve_fraction);
    total_cost <= available_money
}

/// Trait for village decision-making strategies.
///
/// Implementations analyze village and market state to produce:
/// - Worker allocation across food, wood, and construction
/// - Trading orders (bids and asks) for the market
pub trait Strategy: Send + Sync {
    /// Decide worker allocation and market orders based on village state
    fn decide_allocation_and_orders(
        &self,
        village_state: &VillageState,
        market_state: &MarketState,
    ) -> StrategyDecision;

    /// Get a descriptive name for the strategy
    fn name(&self) -> &str;
}

/// Current state of a village for strategy decisions.
///
/// Contains all information strategies need to make informed decisions
/// about resource allocation and trading.
#[derive(Debug, Clone)]
pub struct VillageState {
    pub id: String,
    pub workers: usize,
    pub wood: Decimal,
    pub food: Decimal,
    pub money: Decimal,
    pub houses: usize,
    pub house_capacity: usize,
    pub wood_slots: (u32, u32),
    pub food_slots: (u32, u32),
    pub worker_days: Decimal,
    pub days_without_food: Vec<u32>,
    pub days_without_shelter: Vec<u32>,
    pub construction_progress: Decimal,
}

/// Market information for trading decisions.
///
/// Provides price history and current order book state
/// for both wood and food markets.
#[derive(Debug, Clone)]
pub struct MarketState {
    pub last_wood_price: Option<Decimal>,
    pub last_food_price: Option<Decimal>,
}

/// Strategy output containing allocation and trading decisions.
///
/// All trading orders are optional - strategies only generate
/// orders when they want to participate in the market.
#[derive(Debug, Clone)]
pub struct StrategyDecision {
    pub allocation: WorkerAllocation,
    pub wood_bid: Option<(Decimal, u32)>, // (price, quantity)
    pub wood_ask: Option<(Decimal, u32)>,
    pub food_bid: Option<(Decimal, u32)>,
    pub food_ask: Option<(Decimal, u32)>,
}

/// Worker allocation decision.
///
/// Values represent worker-days to allocate to each task.
/// Should sum to approximately village.worker_days.
#[derive(Debug, Clone)]
pub struct WorkerAllocation {
    pub wood: Decimal,
    pub food: Decimal,
    pub construction: Decimal,
}

// === SURVIVAL STRATEGY ===
/// Prioritizes immediate survival needs with conservative resource management.
///
/// # Philosophy
/// Maintains buffer stocks of resources before pursuing growth. Conservative
/// trading - only buys when critically low, sells when buffers exceed 2x target.
///
/// # Performance
/// - **Excels**: Resource-scarce environments, early game, volatile markets
/// - **Struggles**: Late game growth, high-competition trading scenarios
///
/// # Parameters
/// - `min_food_days`: Target food buffer (default: 20 days)
/// - `min_wood_days`: Target wood buffer (default: 10 days)
pub struct SurvivalStrategy {
    min_food_days: u32,
    min_wood_days: u32,
}

impl SurvivalStrategy {
    pub fn new(min_food_days: u32, min_shelter_buffer: u32) -> Self {
        Self {
            min_food_days,
            min_wood_days: min_shelter_buffer,
        }
    }
}

impl Default for SurvivalStrategy {
    fn default() -> Self {
        Self {
            min_food_days: 20,
            min_wood_days: 10,
        }
    }
}

impl Strategy for SurvivalStrategy {
    fn name(&self) -> &str {
        "Survival"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        market: &MarketState,
    ) -> StrategyDecision {
        let worker_days = village.worker_days;

        // Calculate daily consumption
        let food_per_day = Decimal::from(village.workers);
        let wood_per_day = Decimal::from(village.houses) * dec!(0.1);

        // Calculate days of resources
        let food_days = calculate_resource_days(village.food, food_per_day);
        let wood_days = calculate_resource_days(village.wood, wood_per_day);

        // Allocate workers based on urgency
        let mut allocation = WorkerAllocation {
            wood: dec!(0),
            food: dec!(0),
            construction: dec!(0),
        };

        // Critical food shortage
        if food_days < 5 {
            allocation.food = worker_days;
        }
        // Critical wood shortage
        else if wood_days < 5 {
            allocation.wood = worker_days;
        }
        // Normal allocation
        else {
            let food_weight = if food_days < self.min_food_days {
                0.7
            } else {
                0.4
            };
            let wood_weight = if wood_days < self.min_wood_days {
                0.5
            } else {
                0.3
            };

            let total_weight = food_weight + wood_weight;
            allocation.food = worker_days * Decimal::from_f64(food_weight / total_weight).unwrap();
            allocation.wood = worker_days * Decimal::from_f64(wood_weight / total_weight).unwrap();

            // Only build if we have resource buffer
            if food_days > self.min_food_days && wood_days > self.min_wood_days {
                let construction_allocation =
                    (worker_days * dec!(0.1)).min(worker_days - allocation.food - allocation.wood);
                allocation.construction = construction_allocation;
                allocation.food = allocation.food * (worker_days - construction_allocation)
                    / (allocation.food + allocation.wood);
                allocation.wood = worker_days - allocation.food - allocation.construction;
            }
        }

        // Trading decisions
        let mut wood_bid = None;
        let mut wood_ask = None;
        let mut food_bid = None;
        let mut food_ask = None;

        // Buy food if critically low
        if food_days < 10 && village.money > dec!(20) {
            let quantity = ((self.min_food_days - food_days) * village.workers as u32).min(50);
            let price = calculate_food_bid_price(market.last_food_price, dec!(1.1)); // 10% above market
            if can_afford_quantity(village.money, price, quantity, dec!(0.2)) {
                food_bid = Some((price, quantity));
            } else {
                // Adjust price to what we can afford
                let max_price = village.money / Decimal::from(quantity) * dec!(0.8);
                food_bid = Some((price.min(max_price), quantity));
            }
        }

        // Buy wood if critically low
        if wood_days < 10 && village.money > dec!(20) {
            let quantity = (self.min_wood_days - wood_days).min(20);
            let price = calculate_wood_bid_price(market.last_wood_price, dec!(1.1));
            let max_price = village.money / Decimal::from(quantity) * dec!(0.5);
            wood_bid = Some((price.min(max_price), quantity));
        }

        // Sell excess if we have good buffers
        if food_days > self.min_food_days * 2 {
            let excess = village.food - Decimal::from(self.min_food_days) * food_per_day;
            let quantity = (excess / dec!(2)).to_u32().unwrap_or(0).min(50);
            if quantity > 0 {
                let price = calculate_food_ask_price(market.last_food_price, dec!(0.9));
                food_ask = Some((price, quantity));
            }
        }

        if wood_days > self.min_wood_days * 2 {
            let excess = village.wood - Decimal::from(self.min_wood_days) * wood_per_day;
            let quantity = (excess / dec!(2)).to_u32().unwrap_or(0).min(20);
            if quantity > 0 {
                let price = calculate_wood_ask_price(market.last_wood_price, dec!(0.9));
                wood_ask = Some((price, quantity));
            }
        }

        StrategyDecision {
            allocation,
            wood_bid,
            wood_ask,
            food_bid,
            food_ask,
        }
    }
}

// === GROWTH STRATEGY ===
/// Focuses on population expansion through balanced resource production.
///
/// # Philosophy
/// Maintains optimal worker-to-house ratio for population growth. Trades
/// to acquire resources needed for expansion. Prioritizes long-term growth
/// over short-term efficiency.
///
/// # Performance
/// - **Excels**: Stable markets, mid-to-late game, resource-rich environments
/// - **Struggles**: Early game survival, resource scarcity
///
/// # Parameters
/// - `target_worker_to_house_ratio`: Optimal occupancy (default: 3.5/5.0 = 70%)
pub struct GrowthStrategy {
    target_worker_to_house_ratio: f64,
    house_buffer: usize,
}

impl GrowthStrategy {
    pub fn new(target_population: usize, house_buffer: usize) -> Self {
        // Convert target population to worker-to-house ratio
        // Assuming house capacity of 5, we want ratio that allows for target_population
        let target_ratio = if target_population > 0 {
            (target_population as f64 - house_buffer as f64)
                / (target_population as f64 / 5.0).max(1.0)
        } else {
            3.5
        };
        Self {
            target_worker_to_house_ratio: target_ratio.clamp(2.0, 4.5), // Keep ratio reasonable
            house_buffer,
        }
    }
}

impl Default for GrowthStrategy {
    fn default() -> Self {
        Self {
            target_worker_to_house_ratio: 3.5, // Leave room for growth
            house_buffer: 2,
        }
    }
}

impl Strategy for GrowthStrategy {
    fn name(&self) -> &str {
        "Growth"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        market: &MarketState,
    ) -> StrategyDecision {
        let worker_days = village.worker_days;

        // Calculate if we need more houses, accounting for buffer
        let current_ratio = village.workers as f64 / village.house_capacity.max(1) as f64;
        let available_slots = village.house_capacity.saturating_sub(village.workers);
        let need_houses = current_ratio > self.target_worker_to_house_ratio
            || available_slots < self.house_buffer;

        // Base allocation for growth
        let mut allocation = WorkerAllocation {
            wood: worker_days * dec!(0.3),
            food: worker_days * dec!(0.5),
            construction: if need_houses {
                worker_days * dec!(0.2)
            } else {
                dec!(0)
            },
        };

        // Adjust remaining allocation
        if !need_houses {
            allocation.wood = worker_days * dec!(0.4);
            allocation.food = worker_days * dec!(0.6);
        }

        // Trading - buy resources needed for growth
        let mut wood_bid = None;
        let mut wood_ask = None;
        let mut food_bid = None;
        let food_ask = None;

        // Need wood for construction - buy more aggressively if we're below buffer
        if need_houses && village.wood < dec!(30) && village.money > dec!(50) {
            let urgency_multiplier = if available_slots < self.house_buffer {
                dec!(1.3) // More urgent when below buffer
            } else {
                dec!(1.2)
            };
            let quantity = 20u32;
            let price = calculate_wood_bid_price(market.last_wood_price, urgency_multiplier);
            wood_bid = Some((price, quantity));
        }

        // Need food for population
        let food_per_day = Decimal::from(village.workers);
        let food_days = calculate_resource_days(village.food, food_per_day);
        if food_days < 30 && village.money > dec!(30) {
            let quantity = (30 * village.workers as u32).min(100);
            let price = calculate_food_bid_price(market.last_food_price, dec!(1.15));
            food_bid = Some((price, quantity));
        }

        // Sell excess only if we have plenty
        if village.wood > dec!(100) && !need_houses {
            let quantity = 20u32;
            let price = calculate_wood_ask_price(market.last_wood_price, dec!(0.85));
            wood_ask = Some((price, quantity));
        }

        StrategyDecision {
            allocation,
            wood_bid,
            wood_ask,
            food_bid,
            food_ask,
        }
    }
}

// === TRADING STRATEGY ===
/// Dynamic trading based on marginal cost analysis.
///
/// # Philosophy
/// Calculates the marginal cost of producing each resource based on
/// current allocation and productivity. Sets prices slightly better than
/// break-even to ensure profitable trades. Adjusts each tick based on
/// production costs and market prices.
///
/// # Performance
/// - **Excels**: Active markets, price discovery, efficient allocation
/// - **Struggles**: Isolated play, extreme resource scarcity
///
/// # Pricing
/// - Break-even ratio = (marginal cost of X) / (marginal cost of Y)
/// - Bids at 98% of break-even (slight profit when buying)
/// - Asks at 102% of break-even (slight profit when selling)
pub struct TradingStrategy {
    price_multiplier: Decimal,
    max_trade_fraction: Decimal,
}

impl TradingStrategy {
    pub fn new(price_multiplier: f64, max_trade_fraction: f64) -> Self {
        Self {
            price_multiplier: Decimal::from_f64(price_multiplier).unwrap_or(dec!(1.0)),
            max_trade_fraction: Decimal::from_f64(max_trade_fraction).unwrap_or(dec!(0.3)),
        }
    }
}

impl Default for TradingStrategy {
    fn default() -> Self {
        Self {
            price_multiplier: dec!(1.0),
            max_trade_fraction: dec!(0.3),
        }
    }
}

impl Strategy for TradingStrategy {
    fn name(&self) -> &str {
        "Trading"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        market: &MarketState,
    ) -> StrategyDecision {
        let worker_days = village.worker_days;
        
        // Base production rates (from actual simulation)
        let base_food_rate = dec!(2.0);  // Food per worker-day
        let base_wood_rate = dec!(0.1);  // Wood per worker-day

        // Start with balanced allocation  
        let construction_allocation = worker_days * dec!(0.1);
        let remaining = worker_days - construction_allocation;
        
        // Calculate current marginal costs for initial balanced allocation
        let food_workers_est = (remaining * dec!(0.5)).to_u32().unwrap_or(0);
        let wood_workers_est = (remaining * dec!(0.5)).to_u32().unwrap_or(0);
        
        let food_marginal_cost = calculate_marginal_cost(
            food_workers_est, 
            (village.food_slots.0, village.food_slots.1),
            base_food_rate
        );
        let wood_marginal_cost = calculate_marginal_cost(
            wood_workers_est,
            (village.wood_slots.0, village.wood_slots.1), 
            base_wood_rate
        );
        
        // Break-even exchange rate: How much wood is 1 food worth?
        let wood_per_food_breakeven = food_marginal_cost / wood_marginal_cost;
        
        // Adjust allocation based on which resource is more valuable to produce
        let (food_allocation, wood_allocation) = if food_marginal_cost < wood_marginal_cost {
            // Food is cheaper to produce - allocate more to food
            let food_weight = dec!(0.7);
            let wood_weight = dec!(0.3);
            (remaining * food_weight, remaining * wood_weight)
        } else {
            // Wood is cheaper to produce - allocate more to wood
            let food_weight = dec!(0.3);
            let wood_weight = dec!(0.7);
            (remaining * food_weight, remaining * wood_weight)
        };
        
        let allocation = WorkerAllocation {
            food: food_allocation,
            wood: wood_allocation,
            construction: construction_allocation,
        };

        // Trading based on marginal cost analysis
        let mut wood_bid = None;
        let mut wood_ask = None;
        let mut food_bid = None;
        let mut food_ask = None;
        
        // Use market prices if available, otherwise use break-even ratio
        let _market_wood_per_food = if let (Some(wood_price), Some(food_price)) = 
            (market.last_wood_price, market.last_food_price) {
            if food_price > dec!(0) {
                wood_price / food_price
            } else {
                wood_per_food_breakeven
            }
        } else {
            wood_per_food_breakeven
        };

        // Determine what to trade based on inventory and production efficiency
        let food_days = calculate_resource_days(village.food, Decimal::from(village.workers));
        let wood_days = calculate_resource_days(village.wood, Decimal::from(village.houses) * dec!(0.1));
        
        // If we have excess food and need wood
        if food_days > 20 && wood_days < 15 && village.food > dec!(30) {
            let quantity = (village.food * self.max_trade_fraction)
                .to_u32()
                .unwrap_or(0)
                .min(50);
            if quantity > 0 {
                // Ask slightly above our break-even
                let food_price = if let Some(market_price) = market.last_food_price {
                    market_price * dec!(1.02) * self.price_multiplier
                } else {
                    // Convert break-even ratio to food price
                    dec!(1.0) * dec!(1.02) * self.price_multiplier
                };
                food_ask = Some((food_price, quantity));
            }
        }
        
        // If we have excess wood and need food
        if wood_days > 20 && food_days < 15 && village.wood > dec!(20) {
            let quantity = (village.wood * self.max_trade_fraction)
                .to_u32()
                .unwrap_or(0)
                .min(30);
            if quantity > 0 {
                // Ask slightly above our break-even
                let wood_price = if let Some(market_price) = market.last_wood_price {
                    market_price * dec!(1.02) * self.price_multiplier
                } else {
                    // Use break-even ratio
                    wood_per_food_breakeven * dec!(1.02) * self.price_multiplier
                };
                wood_ask = Some((wood_price, quantity));
            }
        }
        
        // If we urgently need food
        if food_days < 10 && village.money > dec!(20) {
            let quantity = ((15 - food_days) * village.workers as u32).min(50);
            if quantity > 0 {
                // Bid slightly below market/break-even for profit
                let food_price = if let Some(market_price) = market.last_food_price {
                    market_price * dec!(0.98) * self.price_multiplier
                } else {
                    dec!(1.0) * dec!(0.98) * self.price_multiplier
                };
                if can_afford_quantity(village.money, food_price, quantity, dec!(0.2)) {
                    food_bid = Some((food_price, quantity));
                }
            }
        }
        
        // If we urgently need wood
        if wood_days < 10 && village.money > dec!(20) {
            let quantity = (15 - wood_days).min(20);
            if quantity > 0 {
                // Bid slightly below market/break-even for profit
                let wood_price = if let Some(market_price) = market.last_wood_price {
                    market_price * dec!(0.98) * self.price_multiplier  
                } else {
                    wood_per_food_breakeven * dec!(0.98) * self.price_multiplier
                };
                if can_afford_quantity(village.money, wood_price, quantity, dec!(0.2)) {
                    wood_bid = Some((wood_price, quantity));
                }
            }
        }

        StrategyDecision {
            allocation,
            wood_bid,
            wood_ask,
            food_bid,
            food_ask,
        }
    }
}

// === BALANCED STRATEGY ===
/// Adaptive strategy that responds dynamically to current needs.
///
/// # Philosophy
/// Uses urgency-based weighting to allocate workers. More conservative
/// trading with 15-day target buffers. Increases construction when
/// housing becomes limiting factor.
///
/// # Performance
/// - **Excels**: General purpose, unpredictable environments, moderate markets
/// - **Struggles**: Highly competitive specialized markets
///
/// # Adaptation
/// - Food/wood urgency: Inverse of days of supply
/// - Construction: 30% when over capacity, 10% otherwise
pub struct BalancedStrategy {
    food_weight: f64,
    wood_weight: f64,
    construction_weight: f64,
    repair_weight: f64,
}

impl BalancedStrategy {
    pub fn new(
        food_weight: f64,
        wood_weight: f64,
        construction_weight: f64,
        repair_weight: f64,
    ) -> Self {
        Self {
            food_weight,
            wood_weight,
            construction_weight,
            repair_weight,
        }
    }
}

impl Default for BalancedStrategy {
    fn default() -> Self {
        Self {
            food_weight: 0.25,
            wood_weight: 0.25,
            construction_weight: 0.25,
            repair_weight: 0.25,
        }
    }
}

impl Strategy for BalancedStrategy {
    fn name(&self) -> &str {
        "Balanced"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        market: &MarketState,
    ) -> StrategyDecision {
        let worker_days = village.worker_days;

        // Calculate resource needs
        let food_per_day = Decimal::from(village.workers);
        let wood_per_day = Decimal::from(village.houses) * dec!(0.1);

        let food_days = calculate_resource_days(village.food, food_per_day);
        let wood_days = calculate_resource_days(village.wood, wood_per_day);

        // Dynamic weights based on needs and configuration
        let food_urgency = calculate_resource_urgency(food_days, 10.0) * self.food_weight;
        let wood_urgency = calculate_resource_urgency(wood_days, 20.0) * self.wood_weight;

        // Calculate construction need (new houses)
        let new_house_need = if village.workers > village.house_capacity {
            0.3 * self.construction_weight
        } else {
            0.1 * self.construction_weight
        };

        // Calculate repair need based on number of houses
        // More houses = more repair needed
        let repair_need = (village.houses as f64 * 0.02 * self.repair_weight).min(0.2);

        // Combined construction effort for both new houses and repairs
        let construction_need = new_house_need + repair_need;

        let total = food_urgency + wood_urgency + construction_need;

        let allocation = WorkerAllocation {
            food: worker_days * Decimal::from_f64(food_urgency / total).unwrap(),
            wood: worker_days * Decimal::from_f64(wood_urgency / total).unwrap(),
            construction: worker_days * Decimal::from_f64(construction_need / total).unwrap(),
        };

        // Moderate trading
        let mut wood_bid = None;
        let mut wood_ask = None;
        let mut food_bid = None;
        let mut food_ask = None;

        // Buy if below target buffer
        if food_days < 15 && village.money > dec!(30) {
            let quantity = ((15 - food_days) * village.workers as u32).min(50);
            let price = calculate_food_bid_price(market.last_food_price, dec!(1.05));
            food_bid = Some((price, quantity));
        }

        if wood_days < 15 && village.money > dec!(30) {
            let quantity = (15 - wood_days).min(20);
            let price = calculate_wood_bid_price(market.last_wood_price, dec!(1.05));
            wood_bid = Some((price, quantity));
        }

        // Sell if above target buffer
        if food_days > 30 {
            let excess = village.food - dec!(20) * food_per_day;
            let quantity = (excess * dec!(0.5)).to_u32().unwrap_or(0).min(50);
            if quantity > 0 {
                let price = calculate_food_ask_price(market.last_food_price, dec!(0.95));
                food_ask = Some((price, quantity));
            }
        }

        if wood_days > 30 {
            let excess = village.wood - dec!(20) * wood_per_day;
            let quantity = (excess * dec!(0.5)).to_u32().unwrap_or(0).min(20);
            if quantity > 0 {
                let price = calculate_wood_ask_price(market.last_wood_price, dec!(0.95));
                wood_ask = Some((price, quantity));
            }
        }

        StrategyDecision {
            allocation,
            wood_bid,
            wood_ask,
            food_bid,
            food_ask,
        }
    }
}

// === GREEDY STRATEGY ===
/// Maximizes immediate production value with no long-term planning.
///
/// # Philosophy
/// Allocates all workers to highest-value resource based on market prices.
/// No construction. Emergency trades only at premium prices. Sells all
/// surplus aggressively.
///
/// # Performance
/// - **Excels**: Short games, price volatility exploitation, pure production
/// - **Struggles**: Long-term sustainability, population growth, market downturns
///
/// # Trade Behavior
/// - Buys only in emergencies at 150% market price
/// - Sells all surplus at 80% market price
#[derive(Default)]
pub struct GreedyStrategy;

impl Strategy for GreedyStrategy {
    fn name(&self) -> &str {
        "Greedy"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        market: &MarketState,
    ) -> StrategyDecision {
        let worker_days = village.worker_days;

        // Calculate which resource gives more immediate value
        let food_value = dec!(2.0) * market.last_food_price.unwrap_or(dec!(1.0));
        let wood_value = dec!(0.1) * market.last_wood_price.unwrap_or(dec!(5.0));

        // Allocate everything to highest value production
        let allocation = if food_value > wood_value {
            WorkerAllocation {
                wood: dec!(0),
                food: worker_days,
                construction: dec!(0),
            }
        } else {
            WorkerAllocation {
                wood: worker_days,
                food: dec!(0),
                construction: dec!(0),
            }
        };

        // Only trade when desperate
        let mut wood_bid = None;
        let mut wood_ask = None;
        let mut food_bid = None;
        let mut food_ask = None;

        // Emergency buying only
        if village.food < Decimal::from(village.workers) && village.money > dec!(10) {
            let quantity = (village.workers as u32 * 5).min(50);
            let price = calculate_food_bid_price(market.last_food_price, dec!(1.5)); // Will pay premium
            food_bid = Some((price, quantity));
        }

        if village.wood < dec!(1) && village.houses > 0 && village.money > dec!(10) {
            let quantity = 10u32;
            let price = calculate_wood_bid_price(market.last_wood_price, dec!(1.5));
            wood_bid = Some((price, quantity));
        }

        // Sell everything we can
        if village.food > Decimal::from(village.workers * 2) {
            let quantity = (village.food - Decimal::from(village.workers))
                .to_u32()
                .unwrap_or(0)
                .min(100);
            if quantity > 0 {
                let price = calculate_food_ask_price(market.last_food_price, dec!(0.8)); // Will sell cheap
                food_ask = Some((price, quantity));
            }
        }

        if village.wood > dec!(2) {
            let quantity = (village.wood - dec!(1)).to_u32().unwrap_or(0).min(50);
            if quantity > 0 {
                let price = calculate_wood_ask_price(market.last_wood_price, dec!(0.8));
                wood_ask = Some((price, quantity));
            }
        }

        StrategyDecision {
            allocation,
            wood_bid,
            wood_ask,
            food_bid,
            food_ask,
        }
    }
}

// === DEFAULT STRATEGY (legacy) ===
/// Legacy fixed allocation strategy with no trading.
///
/// Simple baseline strategy using fixed percentages:
/// 70% wood, 20% food, 10% construction.
pub struct DefaultStrategy;

impl Strategy for DefaultStrategy {
    fn name(&self) -> &str {
        "Default"
    }

    fn decide_allocation_and_orders(
        &self,
        village: &VillageState,
        _market: &MarketState,
    ) -> StrategyDecision {
        let allocation = WorkerAllocation {
            wood: village.worker_days * dec!(0.7),
            food: village.worker_days * dec!(0.2),
            construction: village.worker_days * dec!(0.1),
        };

        StrategyDecision {
            allocation,
            wood_bid: None,
            wood_ask: None,
            food_bid: None,
            food_ask: None,
        }
    }
}

/// Create a strategy from configuration.
///
/// Used by the scenario system to instantiate strategies
/// with custom parameters.
pub fn create_strategy(config: &StrategyConfig) -> Box<dyn Strategy> {
    match config {
        StrategyConfig::Balanced {
            food_weight,
            wood_weight,
            construction_weight,
            repair_weight,
        } => Box::new(BalancedStrategy::new(
            *food_weight,
            *wood_weight,
            *construction_weight,
            *repair_weight,
        )),
        StrategyConfig::Survival {
            min_food_days,
            min_shelter_buffer,
        } => Box::new(SurvivalStrategy::new(
            *min_food_days as u32,
            *min_shelter_buffer as u32,
        )),
        StrategyConfig::Growth {
            target_population,
            house_buffer,
        } => Box::new(GrowthStrategy::new(*target_population, *house_buffer)),
        StrategyConfig::Trading {
            price_multiplier,
            max_trade_fraction,
        } => Box::new(TradingStrategy::new(*price_multiplier, *max_trade_fraction)),
    }
}

/// Create a strategy by name.
///
/// Used by CLI and testing to create strategies dynamically.
/// Names are case-insensitive.
pub fn create_strategy_by_name(name: &str) -> Box<dyn Strategy> {
    match name.to_lowercase().as_str() {
        "survival" => Box::new(SurvivalStrategy::default()),
        "growth" => Box::new(GrowthStrategy::default()),
        "trading" => Box::new(TradingStrategy::default()),
        "balanced" => Box::new(BalancedStrategy::default()),
        "greedy" => Box::new(GreedyStrategy),
        _ => Box::new(DefaultStrategy),
    }
}
