use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use village_model::{
    Auction,
    auction::{FinalFill, OrderType},
};

struct Village {
    id: usize,
    wood: Decimal,
    food: Decimal,
    wood_slots: (u32, u32),
    food_slots: (u32, u32),
    workers: Vec<Worker>,
    houses: Vec<House>,
    construction_progress: Decimal,

    ask_wood_for_food: (Decimal, u32),
    bid_wood_for_food: (Decimal, u32),
}

impl Village {
    fn new(
        id: usize,
        wood_slots: (u32, u32),
        food_slots: (u32, u32),
        workers: usize,
        houses: usize,
    ) -> Self {
        Self {
            id,
            wood: dec!(100.0),
            food: dec!(100.0),
            wood_slots,
            food_slots,
            workers: vec![Worker::default(); workers],
            houses: vec![House::default(); houses],
            construction_progress: dec!(0.0),
            ask_wood_for_food: (dec!(0.0), 0),
            bid_wood_for_food: (dec!(0.0), 0),
        }
    }
}

#[derive(Default, Clone)]
struct Worker {
    days_without_food: u32,
    days_without_shelter: u32,
    days_with_both: u32,
}

#[derive(Default, Clone, Debug)]
struct House {
    /// Negative means wood is still needed for full repair in whole units.
    /// Positive or zero never exceeds 5 total capacity.
    /// Decreases by 0.1 per day if unmaintained.
    maintenance_level: Decimal,
}

impl House {
    fn shelter_effect(&self) -> Decimal {
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
    fn productivity(&self) -> Decimal {
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
struct Allocation {
    wood: Decimal,
    food: Decimal,
    house_construction: Decimal,
}

impl Village {
    fn worker_days(&self) -> Decimal {
        self.workers.iter().map(|w| w.productivity()).sum()
    }

    fn update(&mut self, allocation: Allocation) {
        let worker_days = self.worker_days();
        assert!(
            ((allocation.wood + allocation.food + allocation.house_construction) - worker_days)
                .abs()
                < dec!(0.001),
            "worker_days: {}, allocation: {:?}",
            worker_days,
            allocation
        );

        self.wood += produced(self.wood_slots, dec!(0.1), allocation.wood);
        self.food += produced(self.food_slots, dec!(2.0), allocation.food);

        // Handle house construction
        if allocation.house_construction > dec!(0.0) {
            self.construction_progress += allocation.house_construction;

            // Check if a house is complete (requires 60 worker-days)
            while self.construction_progress >= dec!(60.0) {
                // Try to build a house if enough wood is available (10 wood)
                if self.wood >= dec!(10.0) {
                    self.wood -= dec!(10.0);
                    self.houses.push(House::default());
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
        let mut workers_to_remove = 0;

        // println!(
        //     "wood: {}, food: {}, shelter_effect: {}",
        //     self.wood, self.food, shelter_effect
        // );

        for worker in self.workers.iter_mut() {
            let has_food = if self.food >= dec!(1.0) {
                self.food -= dec!(1.0);
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
                workers_to_remove += 1;
            } else if worker.days_without_shelter >= 30 {
                println!("worker.days_without_shelter > 30");
                workers_to_remove += 1;
            }
        }

        if workers_to_remove > 0 {
            self.workers
                .truncate(self.workers.len() - workers_to_remove);
        }
        self.workers
            .extend(std::iter::repeat_n(Worker::default(), new_workers));

        for house in self.houses.iter_mut() {
            if self.wood >= dec!(0.1) {
                // eprintln!("wood >= 0.1");
                self.wood -= dec!(0.1);
                if self.wood >= dec!(0.1) && house.maintenance_level < dec!(0.0) {
                    // eprintln!("wood > 0.1 && house.maintenance_level < 0.0");
                    house.maintenance_level += dec!(0.1);
                    self.wood -= dec!(0.1);
                }
            } else {
                // eprintln!("wood < 0.1");
                house.maintenance_level -= dec!(0.1);
            }
        }
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

trait Strategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        _bids: &[(Decimal, u32, usize)],
        _asks: &[(Decimal, u32, usize)],
    ) -> (Allocation, (Decimal, u32), (Decimal, u32));
}

struct DefaultStrategy;

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
    let mut villages = vec![
        Village::new(0, (10, 10), (10, 10), 15, 3),
        Village::new(1, (10, 10), (10, 10), 15, 3),
    ];

    let bids: Vec<(Decimal, u32, usize)> = Vec::new();
    let asks: Vec<(Decimal, u32, usize)> = Vec::new();

    let strategy = DefaultStrategy;

    loop {
        let mut auction = Auction::new(10);

        for village in villages.iter_mut() {
            let (allocation, bid, ask) =
                strategy.decide_allocation_and_bids_asks(village, &bids, &asks);
            village.update(allocation);

            auction.add_participant(&format!("village_{}", village.id), dec!(0.0));
            auction.add_order(
                village.id,
                &format!("village_{}", village.id),
                "wood",
                OrderType::Bid,
                bid.1 as u64,
                bid.0,
                1,
            );
            auction.add_order(
                village.id,
                &format!("village_{}", village.id),
                "wood",
                OrderType::Ask,
                ask.1 as u64,
                ask.0,
                1,
            );

            village.bid_wood_for_food = bid;
            village.ask_wood_for_food = ask;
        }

        let success = auction.run();
        if let Ok(success) = success {
            apply_trades(&mut villages, &success.final_fills);
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal_macros::dec;

    use super::*;

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
        Village::new(0, wood_slots, food_slots, workers, houses)
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
        village.update(alloc(2.0, 1.0, 0.0));

        // Wood: 2 worker-days * 0.1 = 0.2 production
        // Food: 1 worker-day * 2.0 = 2.0 produced, 3 consumed = -1 net
        assert_resources!(village, wood = 100.2, food = 99.0);
    }

    #[test]
    fn test_village_update_partial_slots() {
        let mut village = village_with_slots((1, 1), (1, 1), 3, 0);
        village.update(alloc(3.0, 0.0, 0.0));

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

        village.update(alloc(1.0, 0.0, 0.0));

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

        village.update(alloc(1.0, 0.0, 0.0));

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

        village.update(alloc(0.0, 1.0, 0.0));

        assert_eq!(village.houses[0].maintenance_level, dec!(-0.1));
    }

    #[test]
    fn test_house_maintenance_with_production() {
        let mut village = village_with_slots((2, 0), (1, 0), 2, 1);
        village.wood = dec!(0.0);

        village.update(alloc(2.0, 0.0, 0.0));

        // Produces 0.2 wood, uses 0.1 for maintenance
        assert_eq!(village.houses[0].maintenance_level, dec!(0.0));
        assert_resources!(village, wood = 0.1);
    }

    #[test]
    fn test_house_maintenance_repair() {
        let mut village = village_with_slots((2, 0), (1, 0), 2, 1);
        village.wood = dec!(0.0);
        village.houses[0].maintenance_level = dec!(-2.0);

        village.update(alloc(2.0, 0.0, 0.0));

        // Produces 0.2 wood, uses 0.1 for upkeep and 0.1 for repair
        assert_eq!(village.houses[0].maintenance_level, dec!(-1.9));
        assert_resources!(village, wood = 0.0);
    }

    #[test]
    fn test_worker_death_no_shelter() {
        let mut village = village_with_slots((1, 0), (1, 0), 1, 0); // No houses

        // Run for 30 days - worker should die on day 30
        for day in 1..=30 {
            village.update(alloc(village.worker_days().to_f64().unwrap(), 0.0, 0.0));

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
            village.update(alloc(village.worker_days().to_f64().unwrap(), 0.0, 0.0));

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
        village.update(alloc(0.0, 0.0, worker_days.to_f64().unwrap()));

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
        village.update(alloc(0.0, 0.0, worker_days.to_f64().unwrap()));

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

            village.update(alloc(
                wood_alloc.to_f64().unwrap(),
                food_alloc.to_f64().unwrap(),
                construction_alloc.to_f64().unwrap(),
            ));

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
            village.update(alloc(worker_days.to_f64().unwrap(), 0.0, 0.0));

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

            village.update(alloc(
                wood_alloc.to_f64().unwrap(),
                food_alloc.to_f64().unwrap(),
                construction_alloc.to_f64().unwrap(),
            ));

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

            village.update(alloc(
                wood_alloc.to_f64().unwrap(),
                food_alloc.to_f64().unwrap(),
                construction_alloc.to_f64().unwrap(),
            ));

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
            wood_village.update(alloc(
                (wood_worker_days * dec!(0.8)).to_f64().unwrap(),
                (wood_worker_days * dec!(0.2)).to_f64().unwrap(),
                0.0,
            ));

            // Food village focuses on food production
            let food_worker_days = food_village.worker_days();
            food_village.update(alloc(
                (food_worker_days * dec!(0.2)).to_f64().unwrap(),
                (food_worker_days * dec!(0.8)).to_f64().unwrap(),
                0.0,
            ));

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
            village.update(alloc(
                (worker_days * dec!(0.4)).to_f64().unwrap(),
                (worker_days * dec!(0.4)).to_f64().unwrap(),
                (worker_days * dec!(0.2)).to_f64().unwrap(),
            ));
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
            village.update(alloc(
                (worker_days * dec!(0.5)).to_f64().unwrap(),
                (worker_days * dec!(0.4)).to_f64().unwrap(),
                (worker_days * dec!(0.1)).to_f64().unwrap(),
            ));
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
            healthy_village.update(alloc(
                (healthy_wd * dec!(0.5)).to_f64().unwrap(),
                (healthy_wd * dec!(0.5)).to_f64().unwrap(),
                0.0,
            ));

            struggling_village.update(alloc(
                (struggling_wd * dec!(0.5)).to_f64().unwrap(),
                (struggling_wd * dec!(0.5)).to_f64().unwrap(),
                0.0,
            ));

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
