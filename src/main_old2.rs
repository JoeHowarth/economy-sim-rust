use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::process;
use village_model::{
    Auction,
    auction::{FinalFill, OrderType},
    core::{Allocation, House, Village, Worker},
    events::{ConsumptionPurpose, DeathCause, EventLogger, EventType, ResourceType, TradeSide},
    metrics::MetricsCalculator,
    scenario::{VillageConfig, create_standard_scenarios},
    strategies,
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
    }
}

fn village_from_config(id: usize, config: &VillageConfig) -> Village {
    let workers: Vec<Worker> = (0..config.initial_workers)
        .map(|i| Worker {
            id: i,
            days_without_food: 0,
            days_without_shelter: 0,
            days_with_both: 0,
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
    }
}

fn update_village(
    village: &mut Village,
    allocation: Allocation,
    logger: &mut EventLogger,
    tick: usize,
) {
    let worker_days = village.worker_days();
    assert!(
        ((allocation.wood + allocation.food + allocation.house_construction) - worker_days).abs()
            < dec!(0.001),
        "worker_days: {}, allocation: {:?}",
        worker_days,
        allocation
    );

    // Log worker allocation
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
            repair_workers: 0, // We'll track this separately
            idle_workers,
        },
    );

    // Production
    let wood_produced = produced(village.wood_slots, dec!(0.1), allocation.wood);
    let food_produced = produced(village.food_slots, dec!(2.0), allocation.food);

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
    }

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
    }

    village.wood += wood_produced;
    village.food += food_produced;

    // Handle house construction
    if allocation.house_construction > dec!(0.0) {
        village.construction_progress += allocation.house_construction;

        // Check if a house is complete (requires 60 worker-days)
        while village.construction_progress >= dec!(60.0) {
            // Try to build a house if enough wood is available (10 wood)
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
                println!("New house built! Total houses: {}", village.houses.len());
            } else {
                // Not enough wood, stop construction
                break;
            }
        }
    }

    let mut shelter_effect = village
        .houses
        .iter()
        .map(|h| h.shelter_effect())
        .sum::<Decimal>();
    let mut new_workers = 0;
    let mut workers_to_remove = Vec::new();
    let mut food_consumed = dec!(0);

    for (i, worker) in village.workers.iter_mut().enumerate() {
        let has_food = if village.food >= dec!(1.0) {
            village.food -= dec!(1.0);
            food_consumed += dec!(1.0);
            worker.days_without_food = 0;
            true
        } else {
            worker.days_without_food += 1;
            false
        };

        let has_shelter = shelter_effect >= dec!(1.0);
        if has_shelter {
            shelter_effect -= dec!(1.0);
            worker.days_without_shelter = 0;
        } else {
            worker.days_without_shelter += 1;
        }

        worker.days_with_both = if has_food && has_shelter {
            worker.days_with_both + 1
        } else {
            0
        };

        if worker.days_with_both >= 100 {
            println!("worker.days_with_both >= 100");
            if rand::random_bool(0.05) {
                println!("new worker");
                worker.days_with_both = 0;
                new_workers += 1;
            }
        }
        if worker.days_without_food >= 10 {
            println!("worker.days_without_food > 10");
            workers_to_remove.push((i, worker.id, DeathCause::Starvation));
        } else if worker.days_without_shelter >= 30 {
            println!("worker.days_without_shelter > 30");
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

    // Remove dead workers and log deaths
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

    // Add new workers and log births
    for _ in 0..new_workers {
        let new_worker = Worker {
            id: village.next_worker_id,
            days_without_food: 0,
            days_without_shelter: 0,
            days_with_both: 0,
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

    let mut wood_for_maintenance = dec!(0);
    for house in village.houses.iter_mut() {
        if village.wood >= dec!(0.1) {
            village.wood -= dec!(0.1);
            wood_for_maintenance += dec!(0.1);
            if village.wood >= dec!(0.1) && house.maintenance_level < dec!(0.0) {
                house.maintenance_level += dec!(0.1);
                village.wood -= dec!(0.1);
                wood_for_maintenance += dec!(0.1);
            }
        } else {
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

fn produced(slots: (u32, u32), units_per_slot: Decimal, worker_days: Decimal) -> Decimal {
    let full_slots = Decimal::from(slots.0).min(worker_days);
    let remaining_worker_days = worker_days - full_slots;
    let partial_slots = Decimal::from(slots.1).min(remaining_worker_days);

    (full_slots + partial_slots * dec!(0.5)) * units_per_slot
}

fn apply_trades(
    villages: &mut [Village],
    fills: &[FinalFill],
    logger: &mut EventLogger,
    tick: usize,
) {
    use village_model::auction::OrderType;

    // Process each fill
    for fill in fills {
        // Find the village by matching participant ID
        // The participant ID is created by hashing the village ID string
        let village = villages.iter_mut().find(|v| {
            let id_num = v
                .id_str
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            fill.participant_id.0 == id_num
        });

        if let Some(village) = village {
            let quantity_dec = Decimal::from(fill.filled_quantity);
            let total_value = quantity_dec * fill.price;

            // Update resources based on order type and resource
            match (&fill.order_type, fill.resource_id.0.as_str()) {
                (OrderType::Bid, "wood") => {
                    // Buying wood: spend money, gain wood
                    village.money -= total_value;
                    village.wood += quantity_dec;

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource: ResourceType::Wood,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Buy,
                        },
                    );
                }
                (OrderType::Ask, "wood") => {
                    // Selling wood: gain money, lose wood
                    village.money += total_value;
                    village.wood -= quantity_dec;

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource: ResourceType::Wood,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Sell,
                        },
                    );
                }
                (OrderType::Bid, "food") => {
                    // Buying food: spend money, gain food
                    village.money -= total_value;
                    village.food += quantity_dec;

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource: ResourceType::Food,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Buy,
                        },
                    );
                }
                (OrderType::Ask, "food") => {
                    // Selling food: gain money, lose food
                    village.money += total_value;
                    village.food -= quantity_dec;

                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::TradeExecuted {
                            resource: ResourceType::Food,
                            quantity: quantity_dec,
                            price: fill.price,
                            counterparty: "market".to_string(),
                            side: TradeSide::Sell,
                        },
                    );
                }
                _ => {} // Unknown resource type
            }
        }
    }
}

// Structure to hold all orders from a strategy decision
struct VillageOrders {
    wood_bid: Option<(Decimal, u32)>,
    wood_ask: Option<(Decimal, u32)>,
    food_bid: Option<(Decimal, u32)>,
    food_ask: Option<(Decimal, u32)>,
}

// Adapter to bridge between the new strategies module and village decisions
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
    ) -> (Allocation, VillageOrders) {
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

        // Package all orders
        let orders = VillageOrders {
            wood_bid: decision.wood_bid,
            wood_ask: decision.wood_ask,
            food_bid: decision.food_bid,
            food_ask: decision.food_ask,
        };

        (allocation, orders)
    }
}

fn main() {
    // Parse command line arguments
    use lexopt::prelude::*;

    let mut args = lexopt::Parser::from_env();
    let mut subcommand = None;
    let mut event_file = None;
    let mut strategy_names: Vec<String> = Vec::new();
    let mut scenario_name = "basic".to_string();
    let mut scenario_file = None;

    while let Some(arg) = args.next().unwrap() {
        match arg {
            Value(val) => {
                if subcommand.is_none() {
                    subcommand = Some(val.string().unwrap());
                } else if subcommand.as_deref() == Some("ui") && event_file.is_none() {
                    event_file = Some(val.string().unwrap());
                }
            }
            Long("strategy") | Short('s') => {
                if let Some(Value(val)) = args.next().unwrap() {
                    strategy_names.push(val.string().unwrap());
                }
            }
            Long("scenario") => {
                if let Some(Value(val)) = args.next().unwrap() {
                    scenario_name = val.string().unwrap();
                }
            }
            Long("scenario-file") => {
                if let Some(Value(val)) = args.next().unwrap() {
                    scenario_file = Some(val.string().unwrap());
                }
            }
            Long("help") | Short('h') => {
                print_help();
                return;
            }
            _ => {}
        }
    }

    match subcommand.as_deref() {
        Some("ui") => {
            // Run UI mode
            let file = event_file.unwrap_or_else(|| "simulation_events.json".to_string());
            if let Err(e) = run_ui(&file) {
                eprintln!("Error running UI: {}", e);
                process::exit(1);
            }
        }
        Some("run") | None => {
            // Run simulation (default behavior)
            run_simulation(strategy_names, scenario_name, scenario_file);
        }
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    println!("Village Model Simulation");
    println!();
    println!("USAGE:");
    println!("    village-model-sim [COMMAND] [OPTIONS]");
    println!();
    println!("COMMANDS:");
    println!("    run              Run the simulation (default)");
    println!("    ui [FILE]        View simulation events in TUI");
    println!("                     (default: simulation_events.json)");
    println!();
    println!("OPTIONS:");
    println!("    -s, --strategy <NAME>    Strategy for villages (can be used multiple times)");
    println!("                            Available: default, survival, growth, trading,");
    println!("                            balanced, greedy");
    println!("    --scenario <NAME>        Use a built-in scenario (default: basic)");
    println!("    --scenario-file <FILE>   Load scenario from JSON file");
    println!("    -h, --help              Print help information");
    println!();
    println!("UI CONTROLS:");
    println!("    Space            Pause/Resume playback");
    println!("    ←/→              Step backward/forward through events");
    println!("    Home/End         Jump to beginning/end");
    println!("    +/-              Faster/slower playback (adjust seconds per tick)");
    println!("    Q                Quit");
    println!();
    println!("EXAMPLES:");
    println!("    # Run simulation with default strategies");
    println!("    village-model-sim run");
    println!();
    println!("    # Run with specific strategies for villages");
    println!("    village-model-sim run -s survival -s growth -s trading_wood");
    println!();
    println!("    # Run with a specific scenario");
    println!("    village-model-sim run --scenario competitive");
    println!();
    println!("    # View the simulation in TUI");
    println!("    village-model-sim ui");
}

fn run_simulation(
    strategy_names: Vec<String>,
    scenario_name: String,
    scenario_file: Option<String>,
) {
    // Load scenario
    let scenario = if let Some(file) = scenario_file {
        // Load from file
        match std::fs::read_to_string(&file) {
            Ok(contents) => {
                match serde_json::from_str::<village_model::scenario::Scenario>(&contents) {
                    Ok(scenario) => scenario,
                    Err(e) => {
                        eprintln!("Error parsing scenario file {}: {}", file, e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading scenario file {}: {}", file, e);
                process::exit(1);
            }
        }
    } else {
        // Use built-in scenario
        let scenarios = create_standard_scenarios();
        scenarios.get(&scenario_name).cloned().unwrap_or_else(|| {
            eprintln!("Unknown scenario: {}", scenario_name);
            eprintln!(
                "Available scenarios: {:?}",
                scenarios.keys().collect::<Vec<_>>()
            );
            process::exit(1);
        })
    };

    println!("{}", scenario);

    // Initialize villages from scenario
    let mut villages: Vec<Village> = scenario
        .villages
        .iter()
        .enumerate()
        .map(|(i, config)| village_from_config(i, config))
        .collect();

    // Initialize event logger
    let mut logger = EventLogger::new();

    // Track initial populations for metrics
    let village_configs: Vec<(String, usize)> = villages
        .iter()
        .map(|v| (v.id_str.clone(), v.workers.len()))
        .collect();

    // Print villages with their strategies
    println!("\nVillages with strategies:");

    // Create strategies for each village
    let strategies: Vec<StrategyAdapter> = if strategy_names.is_empty() {
        // Use strategies from scenario configuration
        villages
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let village_state = strategies::VillageState {
                    id: v.id_str.clone(),
                    workers: v.workers.len(),
                    wood: v.wood,
                    food: v.food,
                    money: v.money,
                    houses: v.houses.len(),
                    house_capacity: v.houses.len() * 5,
                    wood_slots: v.wood_slots,
                    food_slots: v.food_slots,
                    worker_days: v.worker_days(),
                    days_without_food: v.workers.iter().map(|w| w.days_without_food).collect(),
                    days_without_shelter: v
                        .workers
                        .iter()
                        .map(|w| w.days_without_shelter)
                        .collect(),
                    construction_progress: v.construction_progress,
                };
                let strategy =
                    strategies::create_strategy(&scenario.villages[i].strategy, &village_state);
                let strategy_name = strategy.name();
                println!("  {}: {} (from scenario)", v.id_str, strategy_name);
                StrategyAdapter::new(strategy)
            })
            .collect()
    } else {
        // Assign strategies in order, cycling if needed
        villages
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let strategy_name = &strategy_names[i % strategy_names.len()];
                println!("  {}: {}", v.id_str, strategy_name);
                let village_state = strategies::VillageState {
                    id: v.id_str.clone(),
                    workers: v.workers.len(),
                    wood: v.wood,
                    food: v.food,
                    money: v.money,
                    houses: v.houses.len(),
                    house_capacity: v.houses.len() * 5,
                    wood_slots: v.wood_slots,
                    food_slots: v.food_slots,
                    worker_days: v.worker_days(),
                    days_without_food: v.workers.iter().map(|w| w.days_without_food).collect(),
                    days_without_shelter: v
                        .workers
                        .iter()
                        .map(|w| w.days_without_shelter)
                        .collect(),
                    construction_progress: v.construction_progress,
                };
                let strategy = strategies::create_strategy_by_name(strategy_name, &village_state);
                StrategyAdapter::new(strategy)
            })
            .collect()
    };

    // Track last clearing prices for strategies
    let mut last_clearing_prices = std::collections::HashMap::<String, Decimal>::new();

    // Run simulation
    for tick in 0..scenario.parameters.days_to_simulate {
        let mut auction = Auction::new(10);

        // Create market state from last clearing prices
        let market_state = strategies::MarketState {
            last_wood_price: last_clearing_prices.get("wood").cloned(),
            last_food_price: last_clearing_prices.get("food").cloned(),
            wood_bids: vec![], // TODO: Could populate from previous tick
            wood_asks: vec![],
            food_bids: vec![],
            food_asks: vec![],
        };

        // Strategy phase
        let mut order_id_counter = 0;
        for (village_idx, village) in villages.iter_mut().enumerate() {
            // Get allocation and orders from strategy
            let (allocation, orders) =
                strategies[village_idx].get_allocation_and_orders(village, &market_state);

            // Update village with event logging
            update_village(village, allocation, &mut logger, tick);

            // Add auction participant
            auction.add_participant(&village.id_str, village.money);

            // Add wood bid
            if let Some((price, quantity)) = orders.wood_bid {
                if quantity > 0 {
                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::OrderPlaced {
                            resource: ResourceType::Wood,
                            quantity: Decimal::from(quantity),
                            price,
                            side: TradeSide::Buy,
                            order_id: format!("{}_wood_bid_{}", village.id_str, tick),
                        },
                    );

                    auction.add_order(
                        order_id_counter,
                        &village.id_str,
                        "wood",
                        OrderType::Bid,
                        quantity as u64,
                        price,
                        tick as u64,
                    );
                    order_id_counter += 1;
                }
            }

            // Add wood ask
            if let Some((price, quantity)) = orders.wood_ask {
                if quantity > 0 {
                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::OrderPlaced {
                            resource: ResourceType::Wood,
                            quantity: Decimal::from(quantity),
                            price,
                            side: TradeSide::Sell,
                            order_id: format!("{}_wood_ask_{}", village.id_str, tick),
                        },
                    );

                    auction.add_order(
                        order_id_counter,
                        &village.id_str,
                        "wood",
                        OrderType::Ask,
                        quantity as u64,
                        price,
                        tick as u64,
                    );
                    order_id_counter += 1;
                }
            }

            // Add food bid
            if let Some((price, quantity)) = orders.food_bid {
                if quantity > 0 {
                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::OrderPlaced {
                            resource: ResourceType::Food,
                            quantity: Decimal::from(quantity),
                            price,
                            side: TradeSide::Buy,
                            order_id: format!("{}_food_bid_{}", village.id_str, tick),
                        },
                    );

                    auction.add_order(
                        order_id_counter,
                        &village.id_str,
                        "food",
                        OrderType::Bid,
                        quantity as u64,
                        price,
                        tick as u64,
                    );
                    order_id_counter += 1;
                }
            }

            // Add food ask
            if let Some((price, quantity)) = orders.food_ask {
                if quantity > 0 {
                    logger.log(
                        tick,
                        village.id_str.clone(),
                        EventType::OrderPlaced {
                            resource: ResourceType::Food,
                            quantity: Decimal::from(quantity),
                            price,
                            side: TradeSide::Sell,
                            order_id: format!("{}_food_ask_{}", village.id_str, tick),
                        },
                    );

                    auction.add_order(
                        order_id_counter,
                        &village.id_str,
                        "food",
                        OrderType::Ask,
                        quantity as u64,
                        price,
                        tick as u64,
                    );
                    order_id_counter += 1;
                }
            }
        }

        // Run auction and process trades
        let auction_result = auction.run();
        if let Ok(success) = auction_result {
            // Update last clearing prices for next tick
            for (resource_id, price) in &success.clearing_prices {
                last_clearing_prices.insert(resource_id.0.clone(), *price);
            }

            // Apply trades to villages
            apply_trades(&mut villages, &success.final_fills, &mut logger, tick);
        }

        // Check for early termination if all villages are dead
        if villages.iter().all(|v| v.workers.is_empty()) {
            println!("All villages have died at tick {}", tick);
            break;
        }
    }

    // Calculate and display metrics
    let metrics = MetricsCalculator::calculate_scenario_metrics(
        logger.get_events(),
        &village_configs,
        scenario.parameters.days_to_simulate,
    );

    println!("\n{}", metrics);

    // Display individual village metrics
    for village_metrics in metrics.villages.values() {
        println!("\n{}", village_metrics);
    }

    // Save events to file
    if let Err(e) = logger.save_to_file("simulation_events.json") {
        eprintln!("Failed to save events: {}", e);
    } else {
        println!("\nEvents saved to simulation_events.json");
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

        // Create a fill for buying wood
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                "village_0"
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
            ),
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: OrderType::Bid,
            filled_quantity: 10,
            price: dec!(15.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &fills, &mut logger, 0);

        // Should have gained 10 wood and lost 150 money
        assert_eq!(villages[0].wood, initial_wood + dec!(10));
        assert_eq!(villages[0].money, initial_money - dec!(150));
    }

    #[test]
    fn test_apply_trades_wood_sell() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        // Create a fill for selling wood
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                "village_0"
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
            ),
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: OrderType::Ask,
            filled_quantity: 5,
            price: dec!(20.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &fills, &mut logger, 0);

        // Should have lost 5 wood and gained 100 money
        assert_eq!(villages[0].wood, initial_wood - dec!(5));
        assert_eq!(villages[0].money, initial_money + dec!(100));
    }

    #[test]
    fn test_apply_trades_food_buy() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        // Create a fill for buying food
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                "village_0"
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
            ),
            resource_id: village_model::auction::ResourceId("food".to_string()),
            order_type: OrderType::Bid,
            filled_quantity: 8,
            price: dec!(12.0),
        }];

        let initial_food = villages[0].food;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &fills, &mut logger, 0);

        // Should have gained 8 food and lost 96 money
        assert_eq!(villages[0].food, initial_food + dec!(8));
        assert_eq!(villages[0].money, initial_money - dec!(96));
    }

    #[test]
    fn test_apply_trades_food_sell() {
        let mut villages = vec![create_village(0, (2, 1), (2, 1), 5, 1)];
        let mut logger = EventLogger::new();

        // Create a fill for selling food
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(
                "village_0"
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
            ),
            resource_id: village_model::auction::ResourceId("food".to_string()),
            order_type: OrderType::Ask,
            filled_quantity: 15,
            price: dec!(10.0),
        }];

        let initial_food = villages[0].food;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &fills, &mut logger, 0);

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

        // Create fills for multiple trades
        let fills = vec![
            // Village 0 buys wood
            FinalFill {
                order_id: village_model::auction::OrderId(1),
                participant_id: village_model::auction::ParticipantId(
                    "village_0"
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
                ),
                resource_id: village_model::auction::ResourceId("wood".to_string()),
                order_type: OrderType::Bid,
                filled_quantity: 10,
                price: dec!(15.0),
            },
            // Village 1 sells wood
            FinalFill {
                order_id: village_model::auction::OrderId(2),
                participant_id: village_model::auction::ParticipantId(
                    "village_1"
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
                ),
                resource_id: village_model::auction::ResourceId("wood".to_string()),
                order_type: OrderType::Ask,
                filled_quantity: 10,
                price: dec!(15.0),
            },
            // Village 0 sells food
            FinalFill {
                order_id: village_model::auction::OrderId(3),
                participant_id: village_model::auction::ParticipantId(
                    "village_0"
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
                ),
                resource_id: village_model::auction::ResourceId("food".to_string()),
                order_type: OrderType::Ask,
                filled_quantity: 5,
                price: dec!(20.0),
            },
            // Village 1 buys food
            FinalFill {
                order_id: village_model::auction::OrderId(4),
                participant_id: village_model::auction::ParticipantId(
                    "village_1"
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_add(b as u32)),
                ),
                resource_id: village_model::auction::ResourceId("food".to_string()),
                order_type: OrderType::Bid,
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

        apply_trades(&mut villages, &fills, &mut logger, 0);

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

        // Create a fill for a non-existent village
        let fills = vec![FinalFill {
            order_id: village_model::auction::OrderId(1),
            participant_id: village_model::auction::ParticipantId(999), // Non-existent
            resource_id: village_model::auction::ResourceId("wood".to_string()),
            order_type: OrderType::Bid,
            filled_quantity: 10,
            price: dec!(15.0),
        }];

        let initial_wood = villages[0].wood;
        let initial_money = villages[0].money;

        apply_trades(&mut villages, &fills, &mut logger, 0);

        // Village 0 should be unchanged
        assert_eq!(villages[0].wood, initial_wood);
        assert_eq!(villages[0].money, initial_money);
    }
}
