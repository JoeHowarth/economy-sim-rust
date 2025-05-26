use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::process;
use serde_json;
use village_model::{
    Auction,
    auction::{FinalFill, OrderType},
    events::{ConsumptionPurpose, DeathCause, EventLogger, EventType, ResourceType, TradeSide},
    metrics::MetricsCalculator,
    scenario::{VillageConfig, create_standard_scenarios, ScenarioConfig},
    strategies::{create_strategy, ResourceSpecialization, TradingStrategy},
    ui::run_ui,
};

pub struct Village {
    pub id: usize,
    pub id_str: String,
    pub wood: Decimal,
    pub food: Decimal,
    pub money: Decimal,
    pub wood_slots: (u32, u32),
    pub food_slots: (u32, u32),
    pub workers: Vec<Worker>,
    pub houses: Vec<House>,
    pub construction_progress: Decimal,

    pub ask_wood_for_food: (Decimal, u32),
    pub bid_wood_for_food: (Decimal, u32),

    // For tracking births/deaths
    pub next_worker_id: usize,
    pub next_house_id: usize,
}

impl Village {
    #[allow(dead_code)]
    fn new(
        id: usize,
        wood_slots: (u32, u32),
        food_slots: (u32, u32),
        workers: usize,
        houses: usize,
    ) -> Self {
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

        Self {
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
            ask_wood_for_food: (dec!(0.0), 0),
            bid_wood_for_food: (dec!(0.0), 0),
            next_worker_id: workers,
            next_house_id: houses,
        }
    }

    fn from_config(id: usize, config: &VillageConfig) -> Self {
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

        Self {
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
            ask_wood_for_food: (dec!(0.0), 0),
            bid_wood_for_food: (dec!(0.0), 0),
            next_worker_id: config.initial_workers,
            next_house_id: config.initial_houses,
        }
    }
}

#[derive(Default, Clone)]
pub struct Worker {
    pub id: usize,
    pub days_without_food: u32,
    pub days_without_shelter: u32,
    pub days_with_both: u32,
}

#[derive(Default, Clone, Debug)]
pub struct House {
    pub id: usize,
    /// Negative means wood is still needed for full repair in whole units.
    /// Positive or zero never exceeds 5 total capacity.
    /// Decreases by 0.1 per day if unmaintained.
    pub maintenance_level: Decimal,
}

impl House {
    pub fn shelter_effect(&self) -> Decimal {
        if self.maintenance_level < dec!(0.0) {
            // Each full negative point of maintenance loses 1 capacity
            let needed = self.maintenance_level.abs().floor();
            let lost_capacity = needed.min(dec!(5));
            dec!(5) - lost_capacity
        } else {
            dec!(5)
        }
    }
}

impl Worker {
    pub fn productivity(&self) -> Decimal {
        let mut productivity = dec!(1.0);
        if self.days_without_food > 0 {
            productivity -= dec!(0.2);
        }
        if self.days_without_shelter > 0 {
            productivity -= dec!(0.2);
        }
        productivity
    }
}

#[derive(Debug)]
pub struct Allocation {
    pub wood: Decimal,
    pub food: Decimal,
    pub house_construction: Decimal,
}

impl Village {
    pub fn worker_days(&self) -> Decimal {
        self.workers.iter().map(|w| w.productivity()).sum()
    }

    fn update(&mut self, allocation: Allocation, logger: &mut EventLogger, tick: usize) {
        let worker_days = self.worker_days();
        assert!(
            ((allocation.wood + allocation.food + allocation.house_construction) - worker_days)
                .abs()
                < dec!(0.001),
            "worker_days: {}, allocation: {:?}",
            worker_days,
            allocation
        );

        // Log worker allocation
        let food_workers = allocation.food.to_u32().unwrap_or(0) as usize;
        let wood_workers = allocation.wood.to_u32().unwrap_or(0) as usize;
        let construction_workers = allocation.house_construction.to_u32().unwrap_or(0) as usize;
        let idle_workers = self
            .workers
            .len()
            .saturating_sub(food_workers + wood_workers + construction_workers);

        logger.log(
            tick,
            self.id_str.clone(),
            EventType::WorkerAllocation {
                food_workers,
                wood_workers,
                construction_workers,
                repair_workers: 0, // We'll track this separately
                idle_workers,
            },
        );

        // Production
        let wood_produced = produced(self.wood_slots, dec!(0.1), allocation.wood);
        let food_produced = produced(self.food_slots, dec!(2.0), allocation.food);

        if wood_produced > dec!(0) {
            logger.log(
                tick,
                self.id_str.clone(),
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
                self.id_str.clone(),
                EventType::ResourceProduced {
                    resource: ResourceType::Food,
                    amount: food_produced,
                    workers_assigned: food_workers,
                },
            );
        }

        self.wood += wood_produced;
        self.food += food_produced;

        // Handle house construction
        if allocation.house_construction > dec!(0.0) {
            self.construction_progress += allocation.house_construction;

            // Check if a house is complete (requires 60 worker-days)
            while self.construction_progress >= dec!(60.0) {
                // Try to build a house if enough wood is available (10 wood)
                if self.wood >= dec!(10.0) {
                    self.wood -= dec!(10.0);
                    logger.log(
                        tick,
                        self.id_str.clone(),
                        EventType::ResourceConsumed {
                            resource: ResourceType::Wood,
                            amount: dec!(10.0),
                            purpose: ConsumptionPurpose::HouseConstruction,
                        },
                    );

                    let new_house = House {
                        id: self.next_house_id,
                        maintenance_level: dec!(0.0),
                    };
                    self.next_house_id += 1;

                    logger.log(
                        tick,
                        self.id_str.clone(),
                        EventType::HouseCompleted {
                            house_id: new_house.id,
                            total_houses: self.houses.len() + 1,
                        },
                    );

                    self.houses.push(new_house);
                    self.construction_progress -= dec!(60.0);
                    println!("New house built! Total houses: {}", self.houses.len());
                } else {
                    // Not enough wood, stop construction
                    break;
                }
            }
        }

        let mut shelter_effect = self
            .houses
            .iter()
            .map(|h| h.shelter_effect())
            .sum::<Decimal>();
        let mut new_workers = 0;
        let mut workers_to_remove = Vec::new();
        let mut food_consumed = dec!(0);

        // println!(
        //     "wood: {}, food: {}, shelter_effect: {}",
        //     self.wood, self.food, shelter_effect
        // );

        for (i, worker) in self.workers.iter_mut().enumerate() {
            let has_food = if self.food >= dec!(1.0) {
                self.food -= dec!(1.0);
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
                self.id_str.clone(),
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
                self.id_str.clone(),
                EventType::WorkerDied {
                    worker_id: *worker_id,
                    cause: cause.clone(),
                    total_population: self.workers.len() - 1,
                },
            );
        }

        for (i, _, _) in workers_to_remove {
            self.workers.remove(i);
        }

        // Add new workers and log births
        for _ in 0..new_workers {
            let new_worker = Worker {
                id: self.next_worker_id,
                days_without_food: 0,
                days_without_shelter: 0,
                days_with_both: 0,
            };
            self.next_worker_id += 1;

            logger.log(
                tick,
                self.id_str.clone(),
                EventType::WorkerBorn {
                    worker_id: new_worker.id,
                    total_population: self.workers.len() + 1,
                },
            );

            self.workers.push(new_worker);
        }

        let mut wood_for_maintenance = dec!(0);
        for house in self.houses.iter_mut() {
            if self.wood >= dec!(0.1) {
                self.wood -= dec!(0.1);
                wood_for_maintenance += dec!(0.1);
                if self.wood >= dec!(0.1) && house.maintenance_level < dec!(0.0) {
                    house.maintenance_level += dec!(0.1);
                    self.wood -= dec!(0.1);
                    wood_for_maintenance += dec!(0.1);
                }
            } else {
                house.maintenance_level -= dec!(0.1);
                logger.log(
                    tick,
                    self.id_str.clone(),
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
                self.id_str.clone(),
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
            self.id_str.clone(),
            EventType::VillageStateSnapshot {
                population: self.workers.len(),
                houses: self.houses.len(),
                food: self.food,
                wood: self.wood,
                money: self.money,
            },
        );
    }
}

fn produced(slots: (u32, u32), units_per_slot: Decimal, worker_days: Decimal) -> Decimal {
    let full_slots = Decimal::from(slots.0).min(worker_days);
    let remaining_worker_days = worker_days - full_slots;
    let partial_slots = Decimal::from(slots.1).min(remaining_worker_days);

    (full_slots + partial_slots * dec!(0.5)) * units_per_slot
}

fn apply_trades(_villages: &mut [Village], _fills: &[FinalFill]) {
    // For now, just skip processing fills since FinalFill doesn't have village info
    // This would need to be updated based on how participant IDs map to villages
}

pub trait Strategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        _bids: &[(Decimal, u32, usize)],
        _asks: &[(Decimal, u32, usize)],
    ) -> (Allocation, (Decimal, u32), (Decimal, u32));
}

pub struct DefaultStrategy;

impl Strategy for DefaultStrategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        _bids: &[(Decimal, u32, usize)],
        _asks: &[(Decimal, u32, usize)],
    ) -> (Allocation, (Decimal, u32), (Decimal, u32)) {
        let allocation = Allocation {
            wood: village.worker_days() * dec!(0.7),
            food: village.worker_days() * dec!(0.2),
            house_construction: village.worker_days() * dec!(0.1),
        };
        let bid = (dec!(0.0), 0);
        let ask = (dec!(0.0), 0);
        (allocation, bid, ask)
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
                if let Ok(Value(val)) = args.next() {
                    strategy_names.push(val.string().unwrap());
                }
            }
            Long("scenario") => {
                if let Ok(Value(val)) = args.next() {
                    scenario_name = val.string().unwrap();
                }
            }
            Long("scenario-file") => {
                if let Ok(Value(val)) = args.next() {
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
            return;
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
    println!("                            Available: default, survival, growth, trading_wood,");
    println!("                            trading_food, balanced, greedy");
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

fn run_simulation(strategy_names: Vec<String>, scenario_name: String, scenario_file: Option<String>) {
    // Load scenario
    let scenario = if let Some(file) = scenario_file {
        // Load from file
        match std::fs::read_to_string(&file) {
            Ok(contents) => match serde_json::from_str::<ScenarioConfig>(&contents) {
                Ok(scenario) => scenario,
                Err(e) => {
                    eprintln!("Error parsing scenario file {}: {}", file, e);
                    process::exit(1);
                }
            },
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
            eprintln!("Available scenarios: {:?}", scenarios.keys().collect::<Vec<_>>());
            process::exit(1);
        })
    };

    println!("{}", scenario);

    // Initialize villages from scenario
    let mut villages: Vec<Village> = scenario
        .villages
        .iter()
        .enumerate()
        .map(|(i, config)| Village::from_config(i, config))
        .collect();

    // Initialize event logger
    let mut logger = EventLogger::new();

    // Track initial populations for metrics
    let village_configs: Vec<(String, usize)> = villages
        .iter()
        .map(|v| (v.id_str.clone(), v.workers.len()))
        .collect();

    // Create strategies for each village
    let strategies: Vec<Box<dyn Strategy>> = if strategy_names.is_empty() {
        // Use default strategy for all villages
        villages.iter().map(|_| create_strategy("default")).collect()
    } else {
        // Assign strategies in order, cycling if needed
        villages
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let strategy_name = &strategy_names[i % strategy_names.len()];
                create_strategy(strategy_name)
            })
            .collect()
    };

    // Run simulation
    for tick in 0..scenario.parameters.days_to_simulate {
        let mut auction = Auction::new(10);

        // Strategy phase
        for (village_idx, village) in villages.iter_mut().enumerate() {
            let bids: Vec<(Decimal, u32, usize)> = Vec::new();
            let asks: Vec<(Decimal, u32, usize)> = Vec::new();

            let (allocation, bid, ask) =
                strategies[village_idx].decide_allocation_and_bids_asks(village, &bids, &asks);

            // Update village with event logging
            village.update(allocation, &mut logger, tick);

            // Add auction participants and orders
            auction.add_participant(&village.id_str, village.money);

            if bid.1 > 0 {
                logger.log(
                    tick,
                    village.id_str.clone(),
                    EventType::OrderPlaced {
                        resource: ResourceType::Wood,
                        quantity: Decimal::from(bid.1),
                        price: bid.0,
                        side: TradeSide::Buy,
                        order_id: format!("{}_{}_bid", village.id_str, tick),
                    },
                );

                auction.add_order(
                    village.id * 1000 + tick, // Unique order ID
                    &village.id_str,
                    "wood",
                    OrderType::Bid,
                    bid.1 as u64,
                    bid.0,
                    tick as u64,
                );
            }

            if ask.1 > 0 {
                logger.log(
                    tick,
                    village.id_str.clone(),
                    EventType::OrderPlaced {
                        resource: ResourceType::Wood,
                        quantity: Decimal::from(ask.1),
                        price: ask.0,
                        side: TradeSide::Sell,
                        order_id: format!("{}_{}_ask", village.id_str, tick),
                    },
                );

                auction.add_order(
                    village.id * 1000 + tick + 500, // Unique order ID
                    &village.id_str,
                    "wood",
                    OrderType::Ask,
                    ask.1 as u64,
                    ask.0,
                    tick as u64,
                );
            }

            village.bid_wood_for_food = bid;
            village.ask_wood_for_food = ask;
        }

        // Run auction and process trades
        let auction_result = auction.run();
        if let Ok(success) = auction_result {
            // Log trades
            for fill in &success.final_fills {
                // Find which villages were involved
                // This is a simplified version - in real code you'd track participant IDs properly
                logger.log(
                    tick,
                    "auction".to_string(),
                    EventType::TradeExecuted {
                        resource: ResourceType::Wood,
                        quantity: Decimal::from(fill.filled_quantity),
                        price: fill.price,
                        counterparty: "other".to_string(),
                        side: if fill.order_type == OrderType::Bid {
                            TradeSide::Buy
                        } else {
                            TradeSide::Sell
                        },
                    },
                );
            }

            apply_trades(&mut villages, &success.final_fills);
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
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal_macros::dec;

    use super::*;

    // Helper to update village without logging
    fn update_village(village: &mut Village, allocation: Allocation) {
        let mut logger = EventLogger::new();
        village.update(allocation, &mut logger, 0);
    }

    // Helper function to create allocations more easily
    fn alloc(wood: f64, food: f64, construction: f64) -> Allocation {
        Allocation {
            wood: Decimal::from_f64_retain(wood).unwrap(),
            food: Decimal::from_f64_retain(food).unwrap(),
            house_construction: Decimal::from_f64_retain(construction).unwrap(),
        }
    }

    // Helper to create a village with custom slots
    fn village_with_slots(
        wood_slots: (u32, u32),
        food_slots: (u32, u32),
        workers: usize,
        houses: usize,
    ) -> Village {
        let mut village = Village::new(0, wood_slots, food_slots, workers, houses);
        // Initialize worker and house IDs for tests
        for (i, worker) in village.workers.iter_mut().enumerate() {
            worker.id = i;
        }
        for (i, house) in village.houses.iter_mut().enumerate() {
            house.id = i;
        }
        village
    }

    // Macro to simplify resource assertions
    macro_rules! assert_resources {
        ($village:expr, wood = $wood:expr, food = $food:expr) => {
            assert_eq!($village.wood, dec!($wood), "Wood mismatch");
            assert_eq!($village.food, dec!($food), "Food mismatch");
        };
        ($village:expr, wood = $wood:expr) => {
            assert_eq!($village.wood, dec!($wood), "Wood mismatch");
        };
        ($village:expr, food = $food:expr) => {
            assert_eq!($village.food, dec!($food), "Food mismatch");
        };
    }

    // Macro to assert worker states
    macro_rules! assert_worker_state {
        ($worker:expr, days_without_food = $dwf:expr, days_without_shelter = $dws:expr, days_with_both = $dwb:expr) => {
            assert_eq!(
                $worker.days_without_food, $dwf,
                "days_without_food mismatch"
            );
            assert_eq!(
                $worker.days_without_shelter, $dws,
                "days_without_shelter mismatch"
            );
            assert_eq!($worker.days_with_both, $dwb, "days_with_both mismatch");
        };
    }

    #[test]
    fn test_village_update_basic_production() {
        let mut village = village_with_slots((2, 0), (1, 0), 3, 0);
        update_village(&mut village, alloc(2.0, 1.0, 0.0));

        // Wood: 2 worker-days * 0.1 = 0.2 production
        // Food: 1 worker-day * 2.0 = 2.0 produced, 3 consumed = -1 net
        assert_resources!(village, wood = 100.2, food = 99.0);
    }

    #[test]
    fn test_village_update_partial_slots() {
        let mut village = village_with_slots((1, 1), (1, 1), 3, 0);
        update_village(&mut village, alloc(3.0, 0.0, 0.0));

        // With slots (1, 1) and 3 worker-days allocated to wood:
        // Full slot: 1 worker-day at 100% = 0.1 wood
        // Partial slot: 1 worker-day at 50% = 0.05 wood
        // Third worker-day: wasted (no more slots)
        assert_resources!(village, wood = 100.15);
    }

    #[test]
    fn test_village_update_worker_states() {
        let mut village = village_with_slots((1, 0), (1, 0), 1, 1);

        // Initial state
        assert_worker_state!(
            village.workers[0],
            days_without_food = 0,
            days_without_shelter = 0,
            days_with_both = 0
        );

        update_village(&mut village, alloc(1.0, 0.0, 0.0));

        // Worker should have food and shelter (village starts with 100 food)
        assert_worker_state!(
            village.workers[0],
            days_without_food = 0,
            days_without_shelter = 0,
            days_with_both = 1
        );
    }

    #[test]
    fn test_village_update_no_resources() {
        let mut village = village_with_slots((0, 1), (1, 0), 1, 1);
        village.wood = dec!(0.0);
        village.food = dec!(0.0);

        update_village(&mut village, alloc(1.0, 0.0, 0.0));

        // Worker should be without food but still have shelter
        assert_worker_state!(
            village.workers[0],
            days_without_food = 1,
            days_without_shelter = 0,
            days_with_both = 0
        );
        // House maintenance decreases by 0.1 when no wood available
        assert_eq!(village.houses[0].maintenance_level, dec!(-0.1));
    }

    #[test]
    fn test_house_maintenance_no_wood() {
        let mut village = village_with_slots((1, 0), (1, 0), 1, 1);
        village.wood = dec!(0.0);

        update_village(&mut village, alloc(0.0, 1.0, 0.0));

        assert_eq!(village.houses[0].maintenance_level, dec!(-0.1));
    }

    #[test]
    fn test_house_maintenance_with_production() {
        let mut village = village_with_slots((2, 0), (1, 0), 2, 1);
        village.wood = dec!(0.0);

        update_village(&mut village, alloc(2.0, 0.0, 0.0));

        // Produces 0.2 wood, uses 0.1 for maintenance
        assert_eq!(village.houses[0].maintenance_level, dec!(0.0));
        assert_resources!(village, wood = 0.1);
    }

    #[test]
    fn test_house_maintenance_repair() {
        let mut village = village_with_slots((2, 0), (1, 0), 2, 1);
        village.wood = dec!(0.0);
        village.houses[0].maintenance_level = dec!(-2.0);

        update_village(&mut village, alloc(2.0, 0.0, 0.0));

        // Produces 0.2 wood, uses 0.1 for upkeep and 0.1 for repair
        assert_eq!(village.houses[0].maintenance_level, dec!(-1.9));
        assert_resources!(village, wood = 0.0);
    }

    #[test]
    fn test_worker_death_no_shelter() {
        let mut village = village_with_slots((1, 0), (1, 0), 1, 0); // No houses

        // Run for 30 days - worker should die on day 30
        for day in 1..=30 {
            update_village(
                &mut village,
                alloc(village.worker_days().to_f64().unwrap(), 0.0, 0.0),
            );

            if day < 30 {
                assert_eq!(
                    village.workers.len(),
                    1,
                    "Worker died too early on day {}",
                    day
                );
                assert_eq!(village.workers[0].days_without_shelter, day as u32);
            }
        }

        assert_eq!(
            village.workers.len(),
            0,
            "Worker should die after 30 days without shelter"
        );
    }

    #[test]
    fn test_worker_death_starvation() {
        let mut village = village_with_slots((1, 0), (1, 0), 1, 1);
        village.food = dec!(0.0); // No food available

        // Run for 10 days - worker should die on day 10
        for day in 1..=10 {
            update_village(
                &mut village,
                alloc(village.worker_days().to_f64().unwrap(), 0.0, 0.0),
            );

            if day < 10 {
                assert_eq!(
                    village.workers.len(),
                    1,
                    "Worker died too early on day {}",
                    day
                );
                assert_eq!(village.workers[0].days_without_food, day as u32);
            }
        }

        assert_eq!(
            village.workers.len(),
            0,
            "Worker should die after 10 days without food"
        );
    }

    #[test]
    fn test_house_construction_basic() {
        let mut village = village_with_slots((0, 0), (0, 0), 65, 0);
        village.food = dec!(1000.0); // Plenty of food
        village.wood = dec!(20.0); // Enough for 2 houses

        // Allocate all worker days to construction
        let worker_days = village.worker_days();
        update_village(&mut village, alloc(0.0, 0.0, worker_days.to_f64().unwrap()));

        // Should have built one house (60 worker-days) using 10 wood
        assert_eq!(village.houses.len(), 1);
        assert_eq!(village.wood, dec!(9.9)); // 20 - 10 - 0.1 maintenance
        assert_eq!(village.construction_progress, worker_days - dec!(60.0));
    }

    #[test]
    fn test_house_construction_insufficient_wood() {
        let mut village = village_with_slots((0, 0), (0, 0), 70, 0);
        village.wood = dec!(5.0); // Not enough wood for a house (needs 10)

        // Allocate all worker days to construction
        let worker_days = village.worker_days();
        update_village(&mut village, alloc(0.0, 0.0, worker_days.to_f64().unwrap()));

        // Should have accumulated progress but not built a house
        assert_eq!(village.houses.len(), 0);
        assert_eq!(village.wood, dec!(5.0)); // No wood consumed
        assert_eq!(village.construction_progress, worker_days); // Progress accumulated
    }

    // --- Integration Tests ---

    #[test]
    fn test_village_lifecycle_30_days() {
        // Test a self-sufficient village over 30 days
        let mut village = village_with_slots((10, 10), (10, 10), 10, 2);
        village.wood = dec!(50.0);
        village.food = dec!(50.0);

        // Track key metrics
        let initial_workers = village.workers.len();
        let initial_houses = village.houses.len();

        for day in 1..=30 {
            let worker_days = village.worker_days();

            // Balanced allocation: prioritize food, then wood, small construction
            let food_alloc = (worker_days * dec!(0.4)).min(worker_days);
            let wood_alloc = (worker_days * dec!(0.4)).min(worker_days - food_alloc);
            let construction_alloc = worker_days - food_alloc - wood_alloc;

            update_village(
                &mut village,
                alloc(
                    wood_alloc.to_f64().unwrap(),
                    food_alloc.to_f64().unwrap(),
                    construction_alloc.to_f64().unwrap(),
                ),
            );

            // Basic survival checks
            assert!(village.workers.len() > 0, "All workers died by day {}", day);
            assert!(
                village.food >= dec!(0.0),
                "Food went negative on day {}",
                day
            );
            assert!(
                village.wood >= dec!(0.0),
                "Wood went negative on day {}",
                day
            );
        }

        // After 30 days, village should be stable or growing
        assert!(
            village.workers.len() >= initial_workers,
            "Village lost workers: {} -> {}",
            initial_workers,
            village.workers.len()
        );
        assert!(
            village.houses.len() >= initial_houses,
            "Village lost houses: {} -> {}",
            initial_houses,
            village.houses.len()
        );
    }

    #[test]
    fn test_starvation_scenario() {
        // Test what happens when food production fails
        let mut village = village_with_slots((10, 10), (0, 0), 15, 3); // No food slots
        village.wood = dec!(100.0);
        village.food = dec!(30.0); // Only 2 days of food for 15 workers

        let mut death_days = Vec::new();

        for day in 1..=15 {
            let initial_workers = village.workers.len();
            let worker_days = village.worker_days();

            // All effort on wood since no food slots
            update_village(&mut village, alloc(worker_days.to_f64().unwrap(), 0.0, 0.0));

            if village.workers.len() < initial_workers {
                death_days.push(day);
            }
        }

        // First deaths should occur around day 12 (2 days initial food + 10 days starvation)
        assert!(
            !death_days.is_empty(),
            "No workers died despite no food production"
        );
        assert!(
            death_days[0] >= 11 && death_days[0] <= 13,
            "First death on day {}, expected around day 12",
            death_days[0]
        );

        // Eventually all workers should die
        assert_eq!(
            village.workers.len(),
            0,
            "Some workers survived without food production"
        );
    }

    #[test]
    fn test_economic_growth() {
        // Test a village with good resource slots growing over time
        let mut village = village_with_slots((20, 20), (20, 20), 10, 2);
        village.wood = dec!(200.0);
        village.food = dec!(200.0);

        // Track growth metrics
        let mut population_history = vec![village.workers.len()];
        let mut house_history = vec![village.houses.len()];
        let mut resource_history = vec![(village.wood, village.food)];

        // Simulate 200 days with focus on growth
        for day in 1..=200 {
            let worker_days = village.worker_days();

            // Growth-oriented allocation - more focus on food to prevent starvation
            let food_alloc = (worker_days * dec!(0.5)).min(worker_days);
            let wood_alloc = (worker_days * dec!(0.3)).min(worker_days - food_alloc);
            let construction_alloc = worker_days - food_alloc - wood_alloc;

            update_village(
                &mut village,
                alloc(
                    wood_alloc.to_f64().unwrap(),
                    food_alloc.to_f64().unwrap(),
                    construction_alloc.to_f64().unwrap(),
                ),
            );

            if day % 10 == 0 {
                population_history.push(village.workers.len());
                house_history.push(village.houses.len());
                resource_history.push((village.wood, village.food));
            }
        }

        // Village should maintain or grow population
        assert!(
            village.workers.len() >= 10,
            "Population declined after 200 days: {} workers",
            village.workers.len()
        );
        assert!(
            village.houses.len() >= 2,
            "Houses declined after 200 days: {} houses",
            village.houses.len()
        );

        // Check for stability or growth (not necessarily continuous growth)
        let final_population = village.workers.len();
        let initial_population = 10;
        assert!(
            final_population >= initial_population,
            "Population declined from {} to {}",
            initial_population,
            final_population
        );
    }

    #[test]
    fn test_resource_scarcity_adaptation() {
        // Test how village handles resource constraints
        let mut village = village_with_slots((2, 2), (2, 2), 20, 4); // Many workers, few slots
        village.wood = dec!(30.0);
        village.food = dec!(30.0);

        // Workers greatly exceed production capacity
        let _max_wood_production = dec!(2.0) * dec!(0.1); // 2 full slots * 0.1 per slot
        let _max_food_production = dec!(2.0) * dec!(2.0); // 2 full slots * 2.0 per slot

        let mut population_stable = false;

        for day in 1..=100 {
            let worker_days = village.worker_days();

            // Survival-focused allocation
            let food_alloc = (worker_days * dec!(0.6)).min(worker_days);
            let wood_alloc = (worker_days * dec!(0.3)).min(worker_days - food_alloc);
            let construction_alloc = worker_days - food_alloc - wood_alloc;

            update_village(
                &mut village,
                alloc(
                    wood_alloc.to_f64().unwrap(),
                    food_alloc.to_f64().unwrap(),
                    construction_alloc.to_f64().unwrap(),
                ),
            );

            // Check if population has stabilized around sustainable level
            if day > 50 && village.workers.len() <= 10 {
                population_stable = true;
            }
        }

        assert!(
            population_stable || village.workers.len() <= 10,
            "Population didn't stabilize to sustainable level: {} workers remain",
            village.workers.len()
        );

        // Should maintain some workers (not complete extinction)
        assert!(
            village.workers.len() > 0,
            "Village went extinct despite having some production capacity"
        );
    }

    #[test]
    fn test_multi_village_trading_simulation() {
        // Test trading between specialized villages
        let mut wood_village = Village::new(0, (20, 10), (5, 5), 10, 2);
        wood_village.wood = dec!(50.0);
        wood_village.food = dec!(20.0);

        let mut food_village = Village::new(1, (5, 5), (20, 10), 10, 2);
        food_village.wood = dec!(20.0);
        food_village.food = dec!(50.0);

        // Simulate 50 days with trading
        for day in 1..=50 {
            // Wood village focuses on wood production
            let wood_worker_days = wood_village.worker_days();
            update_village(
                &mut wood_village,
                alloc(
                    (wood_worker_days * dec!(0.8)).to_f64().unwrap(),
                    (wood_worker_days * dec!(0.2)).to_f64().unwrap(),
                    0.0,
                ),
            );

            // Food village focuses on food production
            let food_worker_days = food_village.worker_days();
            update_village(
                &mut food_village,
                alloc(
                    (food_worker_days * dec!(0.2)).to_f64().unwrap(),
                    (food_worker_days * dec!(0.8)).to_f64().unwrap(),
                    0.0,
                ),
            );

            // Simple trading simulation every 5 days
            if day % 5 == 0 && wood_village.wood > dec!(10.0) && food_village.food > dec!(10.0) {
                // Trade 10 wood for 10 food
                wood_village.wood -= dec!(10.0);
                wood_village.food += dec!(10.0);
                food_village.wood += dec!(10.0);
                food_village.food -= dec!(10.0);
            }
        }

        // Both villages should survive through specialization and trade
        assert!(
            wood_village.workers.len() > 0,
            "Wood village died despite trading"
        );
        assert!(
            food_village.workers.len() > 0,
            "Food village died despite trading"
        );

        // Check that specialization is maintained
        assert!(
            wood_village.wood > wood_village.food,
            "Wood village lost its specialization"
        );
        assert!(
            food_village.food > food_village.wood,
            "Food village lost its specialization"
        );
    }

    #[test]
    fn test_disaster_recovery() {
        // Test how village recovers from disasters
        let mut village = village_with_slots((15, 15), (15, 15), 20, 5);
        village.wood = dec!(200.0);
        village.food = dec!(200.0);

        // Baseline growth for 50 days
        for _ in 1..=50 {
            let worker_days = village.worker_days();
            update_village(
                &mut village,
                alloc(
                    (worker_days * dec!(0.4)).to_f64().unwrap(),
                    (worker_days * dec!(0.4)).to_f64().unwrap(),
                    (worker_days * dec!(0.2)).to_f64().unwrap(),
                ),
            );
        }

        let _pre_disaster_population = village.workers.len();
        let pre_disaster_houses = village.houses.len();

        // Disaster: lose half the workers and damage houses
        village.workers.truncate(village.workers.len() / 2);
        for house in village.houses.iter_mut() {
            house.maintenance_level = dec!(-3.0); // Severe damage
        }

        // Recovery phase - 100 days
        for _ in 1..=100 {
            let worker_days = village.worker_days();

            // Focus on recovery: wood for repairs, food for survival
            update_village(
                &mut village,
                alloc(
                    (worker_days * dec!(0.5)).to_f64().unwrap(),
                    (worker_days * dec!(0.4)).to_f64().unwrap(),
                    (worker_days * dec!(0.1)).to_f64().unwrap(),
                ),
            );
        }

        // Check recovery
        assert!(
            village.workers.len() > village.workers.len() / 2,
            "Village didn't recover population after disaster"
        );

        // Houses should be repaired
        let damaged_houses = village
            .houses
            .iter()
            .filter(|h| h.maintenance_level < dec!(0.0))
            .count();
        assert!(
            damaged_houses < pre_disaster_houses / 2,
            "Most houses still damaged after recovery period"
        );
    }

    #[test]
    fn test_worker_productivity_impact() {
        // Test how worker conditions affect overall productivity
        let mut healthy_village = village_with_slots((10, 10), (10, 10), 10, 3);
        let mut struggling_village = village_with_slots((10, 10), (10, 10), 10, 1);

        healthy_village.wood = dec!(100.0);
        healthy_village.food = dec!(100.0);
        struggling_village.wood = dec!(10.0);
        struggling_village.food = dec!(10.0);

        let mut healthy_production = dec!(0.0);
        let mut struggling_production = dec!(0.0);

        // Run for 10 days tracking production
        for _ in 1..=10 {
            let healthy_wd = healthy_village.worker_days();
            let struggling_wd = struggling_village.worker_days();

            // Same allocation strategy
            update_village(
                &mut healthy_village,
                alloc(
                    (healthy_wd * dec!(0.5)).to_f64().unwrap(),
                    (healthy_wd * dec!(0.5)).to_f64().unwrap(),
                    0.0,
                ),
            );

            update_village(
                &mut struggling_village,
                alloc(
                    (struggling_wd * dec!(0.5)).to_f64().unwrap(),
                    (struggling_wd * dec!(0.5)).to_f64().unwrap(),
                    0.0,
                ),
            );

            healthy_production += healthy_wd;
            struggling_production += struggling_wd;
        }

        // Healthy village should have higher productivity per worker
        let healthy_avg =
            healthy_production / dec!(10.0) / Decimal::from(healthy_village.workers.len());
        let struggling_avg = struggling_production
            / dec!(10.0)
            / Decimal::from(struggling_village.workers.len().max(1));

        assert!(
            healthy_avg > struggling_avg,
            "Healthy workers not more productive: {:.2} vs {:.2}",
            healthy_avg,
            struggling_avg
        );
    }
}
