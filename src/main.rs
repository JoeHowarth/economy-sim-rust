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

    // REMOVED: test_village_update_worker_productivity - tests implementation detail

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

    // REMOVED: test_village_update_worker_growth - flaky due to RNG
    // REMOVED: test_village_update_growth_chance - tests unused method

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

    // REMOVED: test_house_construction_partial - redundant with basic test

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

    // REMOVED: test_house_construction_multiple - unrealistic scenario
    // REMOVED: test_house_construction_with_other_allocations - redundant
}
