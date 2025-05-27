//! Village Model Simulation - A multi-agent economic simulation of village life.
//!
//! # Architecture Overview
//!
//! This simulation models villages as economic agents that:
//! - Allocate workers to resource production (food/wood) and construction
//! - Trade resources through a double auction market
//! - Manage population growth through birth/death mechanics
//! - Balance immediate survival needs with long-term growth
//!
//! ## Core Simulation Loop
//!
//! Each simulation tick:
//! 1. **Strategy Phase**: Villages decide worker allocation and trading orders
//! 2. **Production Phase**: Workers produce resources with diminishing returns
//! 3. **Trading Phase**: Double auction clears buy/sell orders across villages
//! 4. **Consumption Phase**: Workers consume food/shelter, population dynamics occur
//! 5. **Maintenance Phase**: Houses decay and require wood for upkeep
//!
//! ## Key Mechanics
//!
//! - **Production Slots**: Each village has limited high-productivity slots
//!   - First slot: 100% productivity
//!   - Second slot: 50% productivity (diminishing returns)
//!   - Additional workers: 0% productivity
//!
//! - **Worker Lifecycle**:
//!   - Need 1 food/day or begin starving (die after 10 days)
//!   - Need shelter or exposure begins (die after 30 days)
//!   - Spawn new workers with 5% daily chance after 100 days with both resources
//!
//! - **Housing System**:
//!   - Construction: 10 wood + 60 worker-days
//!   - Capacity: 5 workers per house when maintained
//!   - Maintenance: 0.1 wood/tick or house decays

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::process;
use village_model::{
    analysis::{analyze_simulation, compare_simulations, explain_simulation},
    auction::{FinalFill, run_auction},
    auction_builder::AuctionBuilder,
    batch_analysis::{analyze_batch, export_batch_to_csv},
    cli::{Command, apply_overrides, parse_args, validate_scenario},
    core::{Allocation, House, Village, Worker},
    events::{ConsumptionPurpose, DeathCause, EventLogger, EventType, TradeSide},
    experiment::ExperimentBatch,
    metrics::MetricsCalculator,
    query::{export_to_csv as export_query_to_csv, format_query_results, query_events},
    scenario::{VillageConfig, create_standard_scenarios},
    strategies,
    types::{OrderRequest, ResourceType, ResourceTypeExt, VillageId},
    ui::run_ui,
};

// Helper functions to create Villages
#[allow(dead_code)]
fn create_village(
    id: usize,
    wood_slots: (u32, u32),
    food_slots: (u32, u32),
    workers: usize,
    houses: usize,
) -> Village {
    let workers_vec: Vec<Worker> = (0..workers)
        .map(|i| Worker {
            id: i,
            days_without_food: 0,
            days_without_shelter: 0,
            days_with_both: 0,
            spawn_eligible: false,
        })
        .collect();

    let houses_vec: Vec<House> = (0..houses)
        .map(|i| House {
            id: i,
            maintenance_level: dec!(0.0),
        })
        .collect();

    Village {
        id,
        id_str: format!("village_{}", id),
        wood: dec!(100.0),
        food: dec!(100.0),
        money: dec!(100.0),
        wood_slots,
        food_slots,
        workers: workers_vec,
        houses: houses_vec,
        construction_progress: dec!(0.0),
        next_worker_id: workers,
        next_house_id: houses,
        rng: None,
    }
}

fn village_from_config(id: usize, config: &VillageConfig) -> Village {
    let workers: Vec<Worker> = (0..config.initial_workers)
        .map(|i| Worker {
            id: i,
            days_without_food: 0,
            days_without_shelter: 0,
            days_with_both: 0,
            spawn_eligible: false,
        })
        .collect();

    let houses: Vec<House> = (0..config.initial_houses)
        .map(|i| House {
            id: i,
            maintenance_level: dec!(0.0),
        })
        .collect();

    Village {
        id,
        id_str: config.id.clone(),
        wood: config.initial_wood,
        food: config.initial_food,
        money: config.initial_money,
        wood_slots: (config.wood_slots.0 as u32, config.wood_slots.1 as u32),
        food_slots: (config.food_slots.0 as u32, config.food_slots.1 as u32),
        workers,
        houses,
        construction_progress: dec!(0.0),
        next_worker_id: config.initial_workers,
        next_house_id: config.initial_houses,
        rng: None,
    }
}

/// Updates a village for one tick of the simulation.
///
/// This is the core update function that processes all village activities:
/// 1. Validates worker allocation matches available worker-days
/// 2. Processes resource production based on allocation
/// 3. Advances construction progress and completes houses
/// 4. Handles worker feeding, shelter, births, and deaths
/// 5. Maintains houses and handles decay
fn update_village(
    village: &mut Village,
    allocation: Allocation,
    logger: &mut EventLogger,
    tick: usize,
) {
    // Validate allocation matches available worker-days
    let worker_days = village.worker_days();
    assert!(
        ((allocation.wood + allocation.food + allocation.house_construction) - worker_days).abs()
            < dec!(0.001),
        "worker_days: {}, allocation: {:?}",
        worker_days,
        allocation
    );

    log_worker_allocation(village, &allocation, logger, tick);
    process_production(village, &allocation, logger, tick);
    process_construction(village, &allocation, logger, tick);
    let (new_workers, workers_to_remove) = process_worker_lifecycle(village, logger, tick);
    apply_worker_changes(village, new_workers, workers_to_remove, logger, tick);
    process_house_maintenance(village, logger, tick);

    // Log village state snapshot
    logger.log(
        tick,
        village.id_str.clone(),
        EventType::VillageStateSnapshot {
            population: village.workers.len(),
            houses: village.houses.len(),
            food: village.food,
            wood: village.wood,
            money: village.money,
        },
    );
}

/// Logs how workers are allocated across different tasks.
fn log_worker_allocation(
    village: &Village,
    allocation: &Allocation,
    logger: &mut EventLogger,
    tick: usize,
) {
    let food_workers = allocation.food.to_u32().unwrap_or(0) as usize;
    let wood_workers = allocation.wood.to_u32().unwrap_or(0) as usize;
    let construction_workers = allocation.house_construction.to_u32().unwrap_or(0) as usize;
    let idle_workers = village
        .workers
        .len()
        .saturating_sub(food_workers + wood_workers + construction_workers);

    logger.log(
        tick,
        village.id_str.clone(),
        EventType::WorkerAllocation {
            food_workers,
            wood_workers,
            construction_workers,
            repair_workers: 0,
            idle_workers,
        },
    );
}

/// Processes resource production based on worker allocation and production slots.
///
/// Production uses diminishing returns:
/// - First slot workers produce at 100% efficiency
/// - Second slot workers produce at 50% efficiency  
/// - Additional workers produce nothing (0% efficiency)
///
/// Wood production: 0.1 units per worker-day
/// Food production: 2.0 units per worker-day
fn process_production(
    village: &mut Village,
    allocation: &Allocation,
    logger: &mut EventLogger,
    tick: usize,
) {
    let wood_workers = allocation.wood.to_u32().unwrap_or(0) as usize;
    let food_workers = allocation.food.to_u32().unwrap_or(0) as usize;

    // Calculate production with diminishing returns
    let wood_produced = produced(village.wood_slots, dec!(0.1), allocation.wood);
    let food_produced = produced(village.food_slots, dec!(2.0), allocation.food);

    // Log and update wood production
    if wood_produced > dec!(0) {
        logger.log(
            tick,
            village.id_str.clone(),
            EventType::ResourceProduced {
                resource: ResourceType::Wood,
                amount: wood_produced,
                workers_assigned: wood_workers,
            },
        );
        village.wood += wood_produced;
    }

    // Log and update food production
    if food_produced > dec!(0) {
        logger.log(
            tick,
            village.id_str.clone(),
            EventType::ResourceProduced {
                resource: ResourceType::Food,
                amount: food_produced,
                workers_assigned: food_workers,
            },
        );
        village.food += food_produced;
    }
}

/// Processes house construction progress.
///
/// Construction mechanics:
/// - Each worker-day adds 1 progress point
/// - Houses complete at 60 progress points
/// - Completion requires 10 wood (consumed immediately)
/// - Multiple houses can complete in one tick if resources allow
/// - Excess progress carries over to next house
fn process_construction(
    village: &mut Village,
    allocation: &Allocation,
    logger: &mut EventLogger,
    tick: usize,
) {
    if allocation.house_construction <= dec!(0.0) {
        return;
    }

    village.construction_progress += allocation.house_construction;

    // Complete houses when enough progress is accumulated
    while village.construction_progress >= dec!(60.0) {
        // Check if we have enough wood (10 units per house)
        if village.wood >= dec!(10.0) {
            village.wood -= dec!(10.0);
            logger.log(
                tick,
                village.id_str.clone(),
                EventType::ResourceConsumed {
                    resource: ResourceType::Wood,
                    amount: dec!(10.0),
                    purpose: ConsumptionPurpose::HouseConstruction,
                },
            );

            let new_house = House {
                id: village.next_house_id,
                maintenance_level: dec!(0.0),
            };
            village.next_house_id += 1;

            logger.log(
                tick,
                village.id_str.clone(),
                EventType::HouseCompleted {
                    house_id: new_house.id,
                    total_houses: village.houses.len() + 1,
                },
            );

            village.houses.push(new_house);
            village.construction_progress -= dec!(60.0);
        } else {
            // Not enough wood, stop construction
            break;
        }
    }
}

/// Processes worker lifecycle: feeding, shelter, births, and deaths.
///
/// Worker needs and consequences:
/// - Food: 1 unit/day, starve after 10 days without
/// - Shelter: 1 capacity/worker, die from exposure after 30 days without
///
/// Reproduction:
/// - Requires 100+ consecutive days with both food and shelter
/// - 5% daily chance to spawn new worker when conditions met
/// - Resets counter on successful birth
///
/// Returns (new_workers_count, workers_to_remove).
fn process_worker_lifecycle(
    village: &mut Village,
    logger: &mut EventLogger,
    tick: usize,
) -> (usize, Vec<(usize, usize, DeathCause)>) {
    let mut shelter_effect = village
        .houses
        .iter()
        .map(|h| h.shelter_effect())
        .sum::<Decimal>();
    let mut new_workers = 0;
    let mut workers_to_remove = Vec::new();
    let mut food_consumed = dec!(0);

    for (i, worker) in village.workers.iter_mut().enumerate() {
        // Feed workers (1 food per worker per day)
        let has_food = if village.food >= dec!(1.0) {
            village.food -= dec!(1.0);
            food_consumed += dec!(1.0);
            worker.days_without_food = 0;
            true
        } else {
            worker.days_without_food += 1;
            false
        };

        // Provide shelter (1 shelter unit per worker)
        let has_shelter = shelter_effect >= dec!(1.0);
        if has_shelter {
            shelter_effect -= dec!(1.0);
            worker.days_without_shelter = 0;
        } else {
            worker.days_without_shelter += 1;
        }

        // Track days with both food and shelter for reproduction
        worker.days_with_both = if has_food && has_shelter {
            worker.days_with_both + 1
        } else {
            0
        };

        // Mark workers eligible for spawning
        if worker.days_with_both >= 100 {
            worker.spawn_eligible = true;
        }

        // Check for death conditions
        if worker.days_without_food >= 10 {
            workers_to_remove.push((i, worker.id, DeathCause::Starvation));
        } else if worker.days_without_shelter >= 30 {
            workers_to_remove.push((i, worker.id, DeathCause::NoShelter));
        }
    }

    // Log food consumption
    if food_consumed > dec!(0) {
        logger.log(
            tick,
            village.id_str.clone(),
            EventType::ResourceConsumed {
                resource: ResourceType::Food,
                amount: food_consumed,
                purpose: ConsumptionPurpose::WorkerFeeding,
            },
        );
    }

    // Collect eligible workers
    let eligible_count = village.workers.iter().filter(|w| w.spawn_eligible).count();

    // Handle spawning for eligible workers
    for _ in 0..eligible_count {
        if village.should_spawn_worker() {
            // Find the first eligible worker and reset their counter
            if let Some(worker) = village.workers.iter_mut().find(|w| w.spawn_eligible) {
                worker.days_with_both = 0;
                worker.spawn_eligible = false;
                new_workers += 1;
            }
        }
    }

    (new_workers, workers_to_remove)
}

/// Applies worker population changes (births and deaths).
fn apply_worker_changes(
    village: &mut Village,
    new_workers: usize,
    mut workers_to_remove: Vec<(usize, usize, DeathCause)>,
    logger: &mut EventLogger,
    tick: usize,
) {
    // Remove dead workers (process in reverse order to maintain indices)
    workers_to_remove.sort_by_key(|&(i, _, _)| std::cmp::Reverse(i));
    for (_, worker_id, cause) in &workers_to_remove {
        logger.log(
            tick,
            village.id_str.clone(),
            EventType::WorkerDied {
                worker_id: *worker_id,
                cause: cause.clone(),
                total_population: village.workers.len() - 1,
            },
        );
    }

    for (i, _, _) in workers_to_remove {
        village.workers.remove(i);
    }

    // Add new workers
    for _ in 0..new_workers {
        let new_worker = Worker {
            id: village.next_worker_id,
            days_without_food: 0,
            days_without_shelter: 0,
            days_with_both: 0,
            spawn_eligible: false,
        };
        village.next_worker_id += 1;

        logger.log(
            tick,
            village.id_str.clone(),
            EventType::WorkerBorn {
                worker_id: new_worker.id,
                total_population: village.workers.len() + 1,
            },
        );

        village.workers.push(new_worker);
    }
}

/// Processes house maintenance and decay.
///
/// Maintenance mechanics:
/// - Each house requires 0.1 wood/tick for basic upkeep
/// - Houses below 0 maintenance level can be repaired with additional 0.1 wood
/// - Without maintenance, houses decay by 0.1 level/tick
/// - Shelter capacity = 5 * (1 + maintenance_level) when level >= 0
/// - Negative maintenance reduces effective shelter capacity
fn process_house_maintenance(village: &mut Village, logger: &mut EventLogger, tick: usize) {
    let mut wood_for_maintenance = dec!(0);

    for house in village.houses.iter_mut() {
        if village.wood >= dec!(0.1) {
            // Basic maintenance
            village.wood -= dec!(0.1);
            wood_for_maintenance += dec!(0.1);

            // Repair if needed and wood available
            if village.wood >= dec!(0.1) && house.maintenance_level < dec!(0.0) {
                house.maintenance_level += dec!(0.1);
                village.wood -= dec!(0.1);
                wood_for_maintenance += dec!(0.1);
            }
        } else {
            // No wood for maintenance, house decays
            house.maintenance_level -= dec!(0.1);
            logger.log(
                tick,
                village.id_str.clone(),
                EventType::HouseDecayed {
                    house_id: house.id,
                    maintenance_level: house.maintenance_level,
                },
            );
        }
    }

    // Log total wood consumed for maintenance
    if wood_for_maintenance > dec!(0) {
        logger.log(
            tick,
            village.id_str.clone(),
            EventType::ResourceConsumed {
                resource: ResourceType::Wood,
                amount: wood_for_maintenance,
                purpose: ConsumptionPurpose::HouseMaintenance,
            },
        );
    }
}

/// Calculates resource production based on slot allocation and worker assignment.
///
/// Implements diminishing returns:
/// - Full slots (first N): 100% of units_per_slot per worker
/// - Partial slots (next M): 50% of units_per_slot per worker
/// - Beyond slots: 0% productivity
///
/// # Arguments
/// * `slots` - (full_slots, partial_slots) tuple defining productivity tiers
/// * `units_per_slot` - Base production per worker-day at full productivity
/// * `worker_days` - Total worker-days allocated to this resource
fn produced(slots: (u32, u32), units_per_slot: Decimal, worker_days: Decimal) -> Decimal {
    let full_slots = Decimal::from(slots.0).min(worker_days);
    let remaining_worker_days = worker_days - full_slots;
    let partial_slots = Decimal::from(slots.1).min(remaining_worker_days);

    (full_slots + partial_slots * dec!(0.5)) * units_per_slot
}

/// Applies auction results to village inventories.
///
/// Processes each filled order:
/// - Bids (buys): Decrease money, increase resource
/// - Asks (sells): Increase money, decrease resource
///
/// All trades are logged for analysis and metrics.
fn apply_trades(
    villages: &mut [Village],
    village_ids: &HashMap<String, VillageId>,
    fills: &[FinalFill],
    logger: &mut EventLogger,
    tick: usize,
) {
    // Process each fill
    for fill in fills {
        // Find the village by matching participant ID
        let village = villages.iter_mut().find(|v| {
            if let Some(vid) = village_ids.get(&v.id_str) {
                fill.participant_id.0 == vid.to_participant_id()
            } else {
                false
            }
        });

        if let Some(village) = village {
            let quantity_dec = Decimal::from(fill.filled_quantity);
            let total_value = quantity_dec * fill.price;

            // Parse resource type
            let resource =
                ResourceType::from_str(&fill.resource_id.0).unwrap_or(ResourceType::Wood);

            // Update resources based on order type
            match &fill.order_type {
                village_model::auction::OrderType::Bid => {
                    // Buying: spend money, gain resource
                    village.money -= total_value;
                    match resource {
                        ResourceType::Wood => village.wood += quantity_dec,
                        ResourceType::Food => village.food += quantity_dec,
                    }

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Buy,
                        },
                    );
                }
                village_model::auction::OrderType::Ask => {
                    // Selling: gain money, lose resource
                    village.money += total_value;
                    match resource {
                        ResourceType::Wood => village.wood -= quantity_dec,
                        ResourceType::Food => village.food -= quantity_dec,
                    }

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Sell,
                        },
                    );
                }
            }
        }
    }
}

/// Adapter to bridge between the strategies module and village decisions.
///
/// Converts between internal Village representation and the strategy API's
/// VillageState/MarketState abstractions. This allows strategies to be
/// implemented without knowledge of internal simulation details.
struct StrategyAdapter {
    inner: Box<dyn strategies::Strategy>,
}

impl StrategyAdapter {
    fn new(strategy: Box<dyn strategies::Strategy>) -> Self {
        Self { inner: strategy }
    }

    fn get_allocation_and_orders(
        &self,
        village: &Village,
        market_state: &strategies::MarketState,
    ) -> (Allocation, Vec<OrderRequest>) {
        // Convert Village to strategies::VillageState
        let village_state = strategies::VillageState {
            id: village.id_str.clone(),
            workers: village.workers.len(),
            wood: village.wood,
            food: village.food,
            money: village.money,
            houses: village.houses.len(),
            house_capacity: village.houses.len() * 5,
            wood_slots: village.wood_slots,
            food_slots: village.food_slots,
            worker_days: village.worker_days(),
            days_without_food: village
                .workers
                .iter()
                .map(|w| w.days_without_food)
                .collect(),
            days_without_shelter: village
                .workers
                .iter()
                .map(|w| w.days_without_shelter)
                .collect(),
            construction_progress: village.construction_progress,
        };

        // Get decision from strategy
        let decision = self
            .inner
            .decide_allocation_and_orders(&village_state, market_state);

        // Convert allocation
        let allocation = Allocation {
            wood: decision.allocation.wood,
            food: decision.allocation.food,
            house_construction: decision.allocation.construction,
        };

        // Convert orders to requests
        let mut orders = Vec::new();

        if let Some((price, quantity)) = decision.wood_bid {
            orders.push(OrderRequest {
                resource: ResourceType::Wood,
                is_buy: true,
                quantity,
                price,
            });
        }

        if let Some((price, quantity)) = decision.wood_ask {
            orders.push(OrderRequest {
                resource: ResourceType::Wood,
                is_buy: false,
                quantity,
                price,
            });
        }

        if let Some((price, quantity)) = decision.food_bid {
            orders.push(OrderRequest {
                resource: ResourceType::Food,
                is_buy: true,
                quantity,
                price,
            });
        }

        if let Some((price, quantity)) = decision.food_ask {
            orders.push(OrderRequest {
                resource: ResourceType::Food,
                is_buy: false,
                quantity,
                price,
            });
        }

        (allocation, orders)
    }
}

/// Entry point for the village model simulation.
fn main() {
    // Parse enhanced command line arguments
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error parsing arguments: {}", e);
            process::exit(1);
        }
    };

    // Set up logging if debug mode is enabled
    if args.debug {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Execute command
    match args.command {
        Command::Run => {
            run_simulation(args);
        }
        Command::Ui { file } => {
            if let Err(e) = run_ui(&file.to_string_lossy()) {
                eprintln!("Error running UI: {}", e);
                process::exit(1);
            }
        }
        Command::Analyze { file } => match analyze_simulation(&file) {
            Ok(analysis) => {
                println!("\n=== Simulation Analysis ===");
                println!("Total Events: {}", analysis.total_events);
                println!("Duration: {} days", analysis.total_days);
                println!("\nVillage Performance:");
                for village in &analysis.villages {
                    println!(
                        "  {}: {} -> {} workers ({:+.1}% growth)",
                        village.id,
                        village.initial_population,
                        village.final_population,
                        village.growth_rate * 100.0
                    );
                }
                println!("\nMarket Activity:");
                println!("  Orders: {}", analysis.market.total_orders);
                println!(
                    "  Trades: {} ({:.1}% success rate)",
                    analysis.market.total_trades,
                    analysis.market.trade_success_rate * 100.0
                );
                if !analysis.insights.is_empty() {
                    println!("\nInsights:");
                    for insight in &analysis.insights {
                        println!("  - {}", insight);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error analyzing simulation: {}", e);
                process::exit(1);
            }
        },
        Command::Compare { files } => {
            let mut analyses = Vec::new();
            for file in &files {
                match analyze_simulation(file) {
                    Ok(analysis) => analyses.push(analysis),
                    Err(e) => {
                        eprintln!("Error analyzing {}: {}", file.display(), e);
                        process::exit(1);
                    }
                }
            }

            let report = compare_simulations(&analyses);
            println!("\n=== Simulation Comparison ===");
            for (i, summary) in report.simulation_summaries.iter().enumerate() {
                println!("\nSimulation {} ({}):", i + 1, files[i].display());
                println!(
                    "  Avg Growth Rate: {:+.1}%",
                    summary.avg_growth_rate * 100.0
                );
                println!(
                    "  Avg Survival Rate: {:.1}%",
                    summary.avg_survival_rate * 100.0
                );
                println!("  Total Trades: {}", summary.total_trades);
                println!(
                    "  Trade Success Rate: {:.1}%",
                    summary.trade_success_rate * 100.0
                );
            }

            if !report.strategy_rankings.is_empty() {
                println!("\nStrategy Rankings:");
                for (rank, (strategy, score)) in report.strategy_rankings.iter().enumerate() {
                    println!("  {}. {} (score: {:.2})", rank + 1, strategy, score);
                }
            }
        }
        Command::Explain { file } => match analyze_simulation(&file) {
            Ok(analysis) => {
                let explanation = explain_simulation(&analysis);
                println!("{}", explanation);
            }
            Err(e) => {
                eprintln!("Error analyzing simulation: {}", e);
                process::exit(1);
            }
        },
        Command::Batch { config } => {
            match ExperimentBatch::load_from_file(&config) {
                Ok(batch) => {
                    println!("Running {} experiments", batch.experiments.len());
                    if let Some(parallel) = batch.parallel {
                        println!("Parallel execution with {} threads", parallel);
                    }

                    let results = batch.run(args.quiet);

                    // Print summary
                    let successful = results.iter().filter(|r| r.success).count();
                    println!("\n=== Experiment Results ===");
                    println!(
                        "Total: {} | Success: {} | Failed: {}",
                        results.len(),
                        successful,
                        results.len() - successful
                    );

                    for result in &results {
                        if result.success {
                            println!("✓ {} ({} ms)", result.name, result.duration_ms);
                        } else {
                            println!(
                                "✗ {} - {}",
                                result.name,
                                result
                                    .error
                                    .as_ref()
                                    .unwrap_or(&"Unknown error".to_string())
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error loading experiment batch: {}", e);
                    process::exit(1);
                }
            }
        }
        Command::AnalyzeBatch { files, output } => {
            match analyze_batch(&files) {
                Ok(report) => {
                    // Print summary
                    println!("\n=== Batch Analysis Report ===");
                    println!("Analyzed {} simulations", report.simulations.len());
                    println!("\nAggregate Statistics:");
                    println!(
                        "  Mean Growth Rate: {:+.1}% (σ={:.1}%)",
                        report.aggregate_stats.mean_growth_rate * 100.0,
                        report.aggregate_stats.std_growth_rate * 100.0
                    );
                    println!(
                        "  Mean Survival Rate: {:.1}% (σ={:.1}%)",
                        report.aggregate_stats.mean_survival_rate * 100.0,
                        report.aggregate_stats.std_survival_rate * 100.0
                    );
                    println!(
                        "  Mean Trade Volume: {:.1}",
                        report.aggregate_stats.mean_trade_volume
                    );
                    println!(
                        "  Mean Gini Coefficient: {:.3}",
                        report.aggregate_stats.mean_gini
                    );

                    if !report.strategy_performance.is_empty() {
                        println!("\nStrategy Performance:");
                        for (strategy, stats) in &report.strategy_performance {
                            println!(
                                "  {}: {:.1}% growth (n={})",
                                strategy,
                                stats.mean_growth * 100.0,
                                stats.occurrences
                            );
                        }
                    }

                    if !report.insights.is_empty() {
                        println!("\nInsights:");
                        for insight in &report.insights {
                            println!("  - {}", insight);
                        }
                    }

                    // Export to CSV if requested
                    if let Some(output_path) = output {
                        match export_batch_to_csv(&report, &output_path) {
                            Ok(_) => println!("\nResults exported to {}", output_path.display()),
                            Err(e) => eprintln!("Error exporting to CSV: {}", e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error analyzing batch: {}", e);
                    process::exit(1);
                }
            }
        }
        Command::Query { file, filters } => {
            match query_events(&file, &filters) {
                Ok(events) => {
                    let output = format_query_results(&events, args.verbose);
                    println!("{}", output);

                    // Export to CSV if output file specified
                    if let Some(output_path) = args.output_file {
                        match export_query_to_csv(&events, &output_path) {
                            Ok(_) => {
                                println!("\nQuery results exported to {}", output_path.display())
                            }
                            Err(e) => eprintln!("Error exporting to CSV: {}", e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error querying events: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}

/// Runs the main simulation loop.
///
/// # Simulation Flow
///
/// 1. **Initialization**: Load scenario, create villages, assign strategies
/// 2. **Main Loop**: For each tick:
///    - Villages decide allocations and trading orders via strategies
///    - Update villages (production, construction, population)
///    - Run double auction to match orders
///    - Apply trade results to village inventories
/// 3. **Termination**: After N ticks or when all villages die
/// 4. **Output**: Save events to JSON, calculate and display metrics
fn run_simulation(args: village_model::cli::CliArgs) {
    log::info!("Starting simulation with args: {:?}", args);
    // Load scenario
    let mut scenario = if let Some(ref file) = args.scenario_file {
        // Load from file
        match std::fs::read_to_string(file) {
            Ok(contents) => {
                match serde_json::from_str::<village_model::scenario::Scenario>(&contents) {
                    Ok(scenario) => scenario,
                    Err(e) => {
                        eprintln!("Error parsing scenario file {}: {}", file.display(), e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading scenario file {}: {}", file.display(), e);
                process::exit(1);
            }
        }
    } else {
        // Use built-in scenario
        let scenarios = create_standard_scenarios();
        scenarios
            .get(&args.scenario_name)
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("Unknown scenario: {}", args.scenario_name);
                eprintln!(
                    "Available scenarios: {:?}",
                    scenarios.keys().collect::<Vec<_>>()
                );
                process::exit(1);
            })
    };

    // Apply CLI overrides to scenario
    apply_overrides(&mut scenario, &args);

    // Validate scenario configuration
    if !args.quiet {
        validate_scenario(&scenario, &args);
        println!("{}", scenario);
    }

    // Initialize villages from scenario
    let mut villages: Vec<Village> = scenario
        .villages
        .iter()
        .enumerate()
        .map(|(i, config)| village_from_config(i, config))
        .collect();

    // Initialize random number generator if seed provided
    if let Some(seed) = scenario.random_seed {
        log::info!("Using random seed: {}", seed);
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        // Set up RNG for each village with deterministic seeds
        for (i, village) in villages.iter_mut().enumerate() {
            // Create a unique seed for each village based on the base seed
            let village_seed = seed.wrapping_add(i as u64);
            village.rng = Some(StdRng::seed_from_u64(village_seed));
        }
    }

    // Create village ID mapping
    let village_ids: HashMap<String, VillageId> = villages
        .iter()
        .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
        .collect();

    // Track initial populations for metrics
    let village_configs: Vec<(String, usize)> = villages
        .iter()
        .map(|v| (v.id_str.clone(), v.workers.len()))
        .collect();

    // Print villages with their strategies
    if !args.quiet {
        println!("\nVillages with strategies:");
    }

    // Create strategies for each village
    let strategies: Vec<StrategyAdapter> = if args.strategies.is_empty() {
        // Use strategies from scenario configuration
        villages
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let strategy = strategies::create_strategy(&scenario.villages[i].strategy);
                if !args.quiet {
                    println!("  {}: {} (from scenario)", v.id_str, strategy.name());
                }
                StrategyAdapter::new(strategy)
            })
            .collect()
    } else {
        // Assign strategies in order, cycling if needed
        villages
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let strategy_name = &args.strategies[i % args.strategies.len()];
                if !args.quiet {
                    println!("  {}: {}", v.id_str, strategy_name);
                }
                let strategy = strategies::create_strategy_by_name(strategy_name);
                StrategyAdapter::new(strategy)
            })
            .collect()
    };

    // Create event logger
    let mut logger = EventLogger::new();

    // Track last clearing prices for strategies
    let mut last_clearing_prices = HashMap::<village_model::auction::ResourceId, Decimal>::new();

    // Run simulation for configured number of days
    for tick in 0..scenario.parameters.days_to_simulate {
        let mut auction_builder = AuctionBuilder::new();

        // Create market state from last clearing prices
        let market_state = strategies::MarketState {
            last_wood_price: last_clearing_prices
                .get(&village_model::auction::ResourceId("wood".to_string()))
                .cloned(),
            last_food_price: last_clearing_prices
                .get(&village_model::auction::ResourceId("food".to_string()))
                .cloned(),
        };

        // Strategy phase: Each village decides worker allocation and trading orders
        for (village_idx, village) in villages.iter_mut().enumerate() {
            // Get allocation and orders from strategy
            let (allocation, orders) =
                strategies[village_idx].get_allocation_and_orders(village, &market_state);

            // Update village with event logging
            update_village(village, allocation, &mut logger, tick);

            // Add village to auction
            let village_id = &village_ids[&village.id_str];
            auction_builder.add_village(village_id, village.money);

            // Add orders to auction
            for order in orders {
                // Log order
                logger.log(
                    tick,
                    village.id_str.clone(),
                    EventType::OrderPlaced {
                        resource: order.resource,
                        quantity: order.quantity.into(),
                        price: order.price,
                        side: if order.is_buy {
                            TradeSide::Buy
                        } else {
                            TradeSide::Sell
                        },
                        order_id: format!(
                            "{}_{}_{}_{}",
                            village.id_str,
                            order.resource.as_str(),
                            if order.is_buy { "bid" } else { "ask" },
                            tick
                        ),
                    },
                );

                auction_builder.add_order(village_id, order);
            }
        }

        // Run double auction to match buy/sell orders across all villages
        let (orders, participants) = auction_builder.build();
        let auction_result = run_auction(
            orders,
            participants,
            10, // max iterations for price discovery
            last_clearing_prices.clone(),
        );

        if let Ok(success) = auction_result {
            // Update last clearing prices for next tick
            last_clearing_prices = success.clearing_prices.clone();

            // Apply trades to villages
            apply_trades(
                &mut villages,
                &village_ids,
                &success.final_fills,
                &mut logger,
                tick,
            );
        }

        // Check for early termination if all villages have died
        if villages.iter().all(|v| v.workers.is_empty()) {
            if !args.quiet {
                println!("All villages have died at tick {}", tick);
            }
            break;
        }
    }

    // Save events
    let filename = args
        .output_file
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "simulation_events.json".to_string());
    logger.save_to_file(&filename).unwrap();
    if !args.quiet {
        println!("\nEvents saved to {}", filename);
    }

    // Calculate and display metrics
    let metrics = MetricsCalculator::calculate_scenario_metrics(
        logger.get_events(),
        &village_configs,
        scenario.parameters.days_to_simulate,
    );

    if !args.quiet {
        println!("\n{}", metrics);

        // Display individual village metrics
        for village_metrics in metrics.villages.values() {
            println!("\n{}", village_metrics);
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_apply_trades_wood_buy() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create a fill for buying wood
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                village_ids["village_0"].to_participant_id(),
            ),
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: village_model::auction::OrderType::Bid,
            filled_quantity: 10,
            price: dec!(15.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Should have gained 10 wood and lost 150 money
        assert_eq!(villages[0].wood, initial_wood + dec!(10));
        assert_eq!(villages[0].money, initial_money - dec!(150));
    }

    #[test]
    fn test_apply_trades_wood_sell() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create a fill for selling wood
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                village_ids["village_0"].to_participant_id(),
            ),
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: village_model::auction::OrderType::Ask,
            filled_quantity: 5,
            price: dec!(20.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Should have lost 5 wood and gained 100 money
        assert_eq!(villages[0].wood, initial_wood - dec!(5));
        assert_eq!(villages[0].money, initial_money + dec!(100));
    }

    #[test]
    fn test_apply_trades_food_buy() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create a fill for buying food
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                village_ids["village_0"].to_participant_id(),
            ),
            resource_id: village_model::auction::ResourceId("food".to_string()),
            order_type: village_model::auction::OrderType::Bid,
            filled_quantity: 8,
            price: dec!(12.0),
        }];

        let initial_food = villages[0].food;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Should have gained 8 food and lost 96 money
        assert_eq!(villages[0].food, initial_food + dec!(8));
        assert_eq!(villages[0].money, initial_money - dec!(96));
    }

    #[test]
    fn test_apply_trades_food_sell() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create a fill for selling food
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                village_ids["village_0"].to_participant_id(),
            ),
            resource_id: village_model::auction::ResourceId("food".to_string()),
            order_type: village_model::auction::OrderType::Ask,
            filled_quantity: 15,
            price: dec!(10.0),
        }];

        let initial_food = villages[0].food;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Should have lost 15 food and gained 150 money
        assert_eq!(villages[0].food, initial_food - dec!(15));
        assert_eq!(villages[0].money, initial_money + dec!(150));
    }

    #[test]
    fn test_apply_trades_multiple_resources() {
        let mut villages = vec![
            create_village(0, (2, 1), (2, 1), 5, 1),
            create_village(1, (2, 1), (2, 1), 5, 1),
        ];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create fills for multiple trades
        let fills = vec![
            // Village 0 buys wood
            FinalFill {
                order_id: village_model::auction::OrderId(1),
                participant_id: village_model::auction::ParticipantId(
                    village_ids["village_0"].to_participant_id(),
                ),
                resource_id: village_model::auction::ResourceId("wood".to_string()),
                order_type: village_model::auction::OrderType::Bid,
                filled_quantity: 10,
                price: dec!(15.0),
            },
            // Village 1 sells wood
            FinalFill {
                order_id: village_model::auction::OrderId(2),
                participant_id: village_model::auction::ParticipantId(
                    village_ids["village_1"].to_participant_id(),
                ),
                resource_id: village_model::auction::ResourceId("wood".to_string()),
                order_type: village_model::auction::OrderType::Ask,
                filled_quantity: 10,
                price: dec!(15.0),
            },
            // Village 0 sells food
            FinalFill {
                order_id: village_model::auction::OrderId(3),
                participant_id: village_model::auction::ParticipantId(
                    village_ids["village_0"].to_participant_id(),
                ),
                resource_id: village_model::auction::ResourceId("food".to_string()),
                order_type: village_model::auction::OrderType::Ask,
                filled_quantity: 5,
                price: dec!(20.0),
            },
            // Village 1 buys food
            FinalFill {
                order_id: village_model::auction::OrderId(4),
                participant_id: village_model::auction::ParticipantId(
                    village_ids["village_1"].to_participant_id(),
                ),
                resource_id: village_model::auction::ResourceId("food".to_string()),
                order_type: village_model::auction::OrderType::Bid,
                filled_quantity: 5,
                price: dec!(20.0),
            },
        ];

        let v0_initial_wood = villages[0].wood;
        let v0_initial_food = villages[0].food;
        let v0_initial_money = villages[0].money;
        let v1_initial_wood = villages[1].wood;
        let v1_initial_food = villages[1].food;
        let v1_initial_money = villages[1].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Village 0: +10 wood (-150 money), -5 food (+100 money) = net -50 money
        assert_eq!(villages[0].wood, v0_initial_wood + dec!(10));
        assert_eq!(villages[0].food, v0_initial_food - dec!(5));
        assert_eq!(villages[0].money, v0_initial_money - dec!(50));

        // Village 1: -10 wood (+150 money), +5 food (-100 money) = net +50 money
        assert_eq!(villages[1].wood, v1_initial_wood - dec!(10));
        assert_eq!(villages[1].food, v1_initial_food + dec!(5));
        assert_eq!(villages[1].money, v1_initial_money + dec!(50));
    }

    #[test]
    fn test_apply_trades_no_matching_village() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        let village_ids: HashMap<String, VillageId> = villages
            .iter()
            .map(|v| (v.id_str.clone(), VillageId::new(&v.id_str)))
            .collect();

        // Create a fill for a non-existent village
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(999), // Non-existent
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: village_model::auction::OrderType::Bid,
            filled_quantity: 10,
            price: dec!(15.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &village_ids, &fills, &mut logger, 0);

        // Village 0 should be unchanged
        assert_eq!(villages[0].wood, initial_wood);
        assert_eq!(villages[0].money, initial_money);
    }
}
