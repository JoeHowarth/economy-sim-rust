use std::collections::HashMap;

struct Village {
    id: usize,
    wood: f64,
    food: f64,
    wood_slots: (u32, u32),
    food_slots: (u32, u32),
    workers: Vec<Worker>,
    houses: Vec<House>,
    construction_progress: f64,

    ask_wood_for_food: (f64, u32),
    bid_wood_for_food: (f64, u32),
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
            wood: 100.0,
            food: 100.0,
            wood_slots,
            food_slots,
            workers: vec![Worker::default(); workers],
            houses: vec![House::default(); houses],
            construction_progress: 0.0,
            ask_wood_for_food: (0.0, 0),
            bid_wood_for_food: (0.0, 0),
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
    maintenance_level: f64,
}

impl House {
    fn shelter_effect(&self) -> f64 {
        if self.maintenance_level < 0.0 {
            // Each full negative point of maintenance loses 1 capacity
            let needed = self.maintenance_level.abs().floor() as u32;
            let lost_capacity = needed.min(5);
            (5 - lost_capacity) as f64
        } else {
            5.0
        }
    }
}

impl Worker {
    fn productivity(&self) -> f64 {
        let mut productivity = 1.0;
        if self.days_without_food > 0 {
            productivity -= 0.2;
        }
        if self.days_without_shelter > 0 {
            productivity -= 0.2;
        }
        productivity
    }

    fn growth_chance(&self) -> f64 {
        if self.days_with_both > 100 { 0.05 } else { 0.0 }
    }
}

#[derive(Debug)]
struct Allocation {
    wood: f64,
    food: f64,
    house_construction: f64,
}

impl Village {
    fn worker_days(&self) -> f64 {
        self.workers.iter().map(|w| w.productivity()).sum()
    }

    fn update(&mut self, allocation: Allocation) {
        let worker_days = self.worker_days();
        assert!(
            ((allocation.wood + allocation.food + allocation.house_construction) - worker_days)
                .abs()
                < 0.001,
            "worker_days: {}, allocation: {:?}",
            worker_days,
            allocation
        );

        self.wood += produced(self.wood_slots, 0.1, allocation.wood);
        self.food += produced(self.food_slots, 2.0, allocation.food);

        // Handle house construction
        if allocation.house_construction > 0.0 {
            self.construction_progress += allocation.house_construction;

            // Check if a house is complete (requires 60 worker-days)
            while self.construction_progress >= 60.0 {
                // Try to build a house if enough wood is available (10 wood)
                if self.wood >= 10.0 {
                    self.wood -= 10.0;
                    self.houses.push(House::default());
                    self.construction_progress -= 60.0;
                    println!("New house built! Total houses: {}", self.houses.len());
                } else {
                    // Not enough wood, stop construction
                    break;
                }
            }
        }

        let mut shelter_effect = self.houses.iter().map(|h| h.shelter_effect()).sum::<f64>();
        let mut new_workers = 0;
        let mut workers_to_remove = 0;

        println!(
            "wood: {}, food: {}, shelter_effect: {}",
            self.wood, self.food, shelter_effect
        );

        for worker in self.workers.iter_mut() {
            let has_food = if self.food >= 1.0 {
                self.food -= 1.0;
                worker.days_without_food = 0;
                true
            } else {
                worker.days_without_food += 1;
                false
            };

            let has_shelter = shelter_effect >= 1.0;
            if has_shelter {
                shelter_effect -= 1.0;
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
            .extend(std::iter::repeat(Worker::default()).take(new_workers));

        for house in self.houses.iter_mut() {
            if self.wood >= 0.1 {
                // eprintln!("wood >= 0.1");
                self.wood -= 0.1;
                if self.wood >= 0.1 && house.maintenance_level < 0.0 {
                    // eprintln!("wood > 0.1 && house.maintenance_level < 0.0");
                    house.maintenance_level += 0.1;
                    self.wood -= 0.1;
                }
            } else {
                // eprintln!("wood < 0.1");
                house.maintenance_level -= 0.1;
            }
        }
    }
}

fn produced(slots: (u32, u32), units_per_slot: f64, worker_days: f64) -> f64 {
    let full_slots = (slots.0 as f64).min(worker_days);
    let remaining_worker_days = worker_days - full_slots;
    let partial_slots = (slots.1 as f64).min(remaining_worker_days);

    (full_slots + partial_slots * 0.5) * units_per_slot
}

#[derive(Debug, PartialEq)]
struct Trade {
    ask_village_id: usize,
    bid_village_id: usize,
    price: f64,
    quantity: f64,
}

fn apply_trades(villages: &mut [Village], trades: &[Trade]) {
    for trade in trades {
        let ask_village = &mut villages[trade.ask_village_id];
        ask_village.wood -= trade.quantity;

        let bid_village = &mut villages[trade.bid_village_id];
        bid_village.wood += trade.quantity;
    }
}

trait Strategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        bids: &[(f64, u32, usize)],
        asks: &[(f64, u32, usize)],
    ) -> (Allocation, (f64, u32), (f64, u32));
}

struct DefaultStrategy;

impl Strategy for DefaultStrategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        bids: &[(f64, u32, usize)],
        asks: &[(f64, u32, usize)],
    ) -> (Allocation, (f64, u32), (f64, u32)) {
        let allocation = Allocation {
            wood: village.worker_days() * 0.7,
            food: village.worker_days() * 0.2,
            house_construction: village.worker_days() * 0.1,
        };
        let bid = (0.0, 0);
        let ask = (0.0, 0);
        (allocation, bid, ask)
    }
}

fn main() {
    let mut villages = vec![
        Village::new(0, (10, 10), (10, 10), 15, 3),
        Village::new(1, (10, 10), (10, 10), 15, 3),
    ];

    let mut trades = Vec::new();
    let mut bids: Vec<(f64, u32, usize)> = Vec::new();
    let mut asks: Vec<(f64, u32, usize)> = Vec::new();

    let strategy = DefaultStrategy;

    loop {
        for village in villages.iter_mut() {
            let (allocation, bid, ask) =
                strategy.decide_allocation_and_bids_asks(&village, &bids, &asks);
            village.update(allocation);
            village.bid_wood_for_food = bid;
            village.ask_wood_for_food = ask;
        }

        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        trades = match_trades(&asks, &bids);
        apply_trades(&mut villages, &trades);
    }
}

fn gather_bids_and_asks(
    villages: &[Village],
    asks: &mut Vec<(f64, u32, usize)>,
    bids: &mut Vec<(f64, u32, usize)>,
) {
    asks.clear();
    asks.extend(
        villages
            .iter()
            .map(|v| (v.ask_wood_for_food.0, v.ask_wood_for_food.1, v.id)),
    );
    asks.sort_by_key(|(p, q, _)| (p * 1000.) as i32 + *q as i32);

    bids.clear();
    bids.extend(
        villages
            .iter()
            .map(|v| (v.bid_wood_for_food.0, v.bid_wood_for_food.1, v.id)),
    );
    bids.sort_by_key(|(p, q, _)| (-p * 1000.) as i32 - *q as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_village(
        id: usize,
        ask_wood_for_food: (f64, u32),
        bid_wood_for_food: (f64, u32),
    ) -> Village {
        Village {
            id,
            wood: 100.0,
            food: 100.0,
            wood_slots: (10, 10),
            food_slots: (10, 10),
            workers: vec![],
            houses: vec![],
            construction_progress: 0.0,
            ask_wood_for_food,
            bid_wood_for_food,
        }
    }

    #[test]
    fn test_match_trades_basic() {
        let mut villages = vec![
            base_village(0, (2.0, 5), (0.0, 0)),
            base_village(1, (0.0, 0), (4.0, 4)),
        ];

        // expected trade:
        // if bid is above ask, go with ask + bid / 2.0
        // price at 3, exchange 4 units, A gets 4 food, B gets 12 wood

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        let expected_trades = vec![Trade {
            ask_village_id: 0,
            bid_village_id: 1,
            price: 3.0,
            quantity: 4.0,
        }];

        assert_eq!(trades, expected_trades);
    }

    #[test]
    fn test_match_trades_multiple_villages() {
        let villages = vec![
            base_village(0, (2.0, 5), (0.0, 0)), // Selling wood at 2.0
            base_village(1, (0.0, 0), (4.0, 4)), // Buying wood at 4.0
            base_village(2, (1.5, 3), (0.0, 0)), // Selling wood at 1.5 (better price)
            base_village(3, (0.0, 0), (3.0, 6)), // Buying wood at 3.0
        ];

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        // Expected trades:
        // Village 2 (ask 1.5) with Village 1 (bid 4.0) - price 2.75, quantity 3.0
        // Village 0 (ask 2.0) with Village 1 (bid 4.0) - price 3.0, quantity 1.0
        // Village 0 (ask 2.0) with Village 3 (bid 3.0) - price 2.5, quantity 4.0
        let expected_trades = vec![
            Trade {
                ask_village_id: 2,
                bid_village_id: 1,
                price: 2.75,
                quantity: 3.0,
            },
            Trade {
                ask_village_id: 0,
                bid_village_id: 1,
                price: 3.0,
                quantity: 1.0,
            },
            Trade {
                ask_village_id: 0,
                bid_village_id: 3,
                price: 2.5,
                quantity: 4.0,
            },
        ];

        assert_eq!(trades, expected_trades);
    }

    #[test]
    fn test_match_trades_no_matches() {
        let villages = vec![
            base_village(0, (3.0, 5), (0.0, 0)), // Selling wood at 3.0
            base_village(1, (0.0, 0), (2.0, 4)), // Buying wood at 2.0 (too low)
        ];

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        // No trades should occur since bid price is below ask price
        assert_eq!(trades, vec![]);
    }

    #[test]
    fn test_match_trades_zero_quantities() {
        let villages = vec![
            base_village(0, (2.0, 0), (0.0, 0)), // Selling wood but quantity is 0
            base_village(1, (0.0, 0), (4.0, 0)), // Buying wood but quantity is 0
            base_village(2, (1.5, 3), (0.0, 0)), // Valid seller
            base_village(3, (0.0, 0), (3.0, 4)), // Valid buyer
        ];

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        // Only one trade should occur between valid seller and buyer
        let expected_trades = vec![Trade {
            ask_village_id: 2,
            bid_village_id: 3,
            price: 2.25,
            quantity: 3.0,
        }];

        assert_eq!(trades, expected_trades);
    }

    #[test]
    fn test_match_trades_exact_price_match() {
        let villages = vec![
            base_village(0, (3.0, 5), (0.0, 0)), // Selling at 3.0
            base_village(1, (0.0, 0), (3.0, 4)), // Buying at 3.0
        ];

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        // No trades should occur since bid price equals ask price (not greater than)
        assert_eq!(trades, vec![]);
    }

    #[test]
    fn test_match_trades_partial_quantities() {
        let villages = vec![
            base_village(0, (2.0, 10), (0.0, 0)), // Selling 10 units at 2.0
            base_village(1, (0.0, 0), (4.0, 3)),  // Buying 3 units at 4.0
            base_village(2, (0.0, 0), (3.0, 5)),  // Buying 5 units at 3.0
        ];

        let mut asks = Vec::new();
        let mut bids = Vec::new();
        gather_bids_and_asks(&villages, &mut asks, &mut bids);
        let trades = match_trades(&asks, &bids);

        // Village 0 should sell 3 units to Village 1 and 5 units to Village 2
        let expected_trades = vec![
            Trade {
                ask_village_id: 0,
                bid_village_id: 1,
                price: 3.0,
                quantity: 3.0,
            },
            Trade {
                ask_village_id: 0,
                bid_village_id: 2,
                price: 2.5,
                quantity: 5.0,
            },
        ];

        assert_eq!(trades, expected_trades);
    }

    #[test]
    fn test_village_update_basic_production() {
        let mut village = Village::new(0, (2, 0), (1, 0), 3, 0);
        let allocation = Allocation {
            wood: 2.0,
            food: 1.0,
            house_construction: 0.0,
        };

        village.update(allocation);

        // Each full slot produces 0.1 wood per worker day
        // 2 workers * 1.0 productivity filling 2 full slots * 0.1 = 0.2 wood
        assert_eq!(village.wood, 100.2);
        // Each full slot produces 1.0 food per worker day
        // 2 workers * 1.0 productivity * 1 full slot * 1.0 = 2.0 food
        assert_eq!(village.food, 99.0); // 100 - 3.0 consumed + 2.0 produced
    }

    #[test]
    fn test_village_update_partial_slots() {
        let mut village = Village::new(0, (1, 1), (1, 1), 3, 0);
        let allocation = Allocation {
            wood: 3.0,
            food: 0.0,
            house_construction: 0.0,
        };

        village.update(allocation);

        // Full slot: 1 slot * 0.1 wood * 1.0 productivity = 0.1
        // Partial slot: 1 slot * 0.5 * 0.1 wood * 1.0 productivity = 0.05
        // Total wood production: 0.15
        assert_eq!(village.wood, 100.15);
    }

    #[test]
    fn test_village_update_worker_states() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 1);
        let allocation = Allocation {
            wood: 1.0,
            food: 0.0,
            house_construction: 0.0,
        };

        // Initial state
        assert_eq!(village.workers[0].days_without_food, 0);
        assert_eq!(village.workers[0].days_without_shelter, 0);
        assert_eq!(village.workers[0].days_with_both, 0);

        village.update(allocation);

        // Worker should have food and shelter
        assert_eq!(village.workers[0].days_without_food, 0);
        assert_eq!(village.workers[0].days_without_shelter, 0);
        assert_eq!(village.workers[0].days_with_both, 1);
    }

    #[test]
    fn test_village_update_no_resources() {
        let mut village = Village::new(0, (0, 1), (1, 0), 1, 1);
        village.wood = 0.0;
        village.food = 0.0;

        let allocation = Allocation {
            wood: 1.0,
            food: 0.0,
            house_construction: 0.0,
        };

        village.update(allocation);

        // Worker should be without food and shelter
        assert_eq!(village.workers[0].days_without_food, 1);
        assert_eq!(village.workers[0].days_without_shelter, 0);
        assert_eq!(village.workers[0].days_with_both, 0);
        assert_eq!(village.houses[0].maintenance_level, -0.1);
    }

    #[test]
    fn test_village_update_house_maintenance() {
        {
            println!("\ntest_village_update_house_maintenance 1");
            let mut village = Village::new(0, (1, 0), (1, 0), 1, 1);
            village.wood = 0.0;

            let allocation = Allocation {
                wood: 0.0,
                food: 1.0,
                house_construction: 0.0,
            };

            village.update(allocation);

            // House maintenance should decrease by 0.1 when no wood available
            assert_eq!(village.houses[0].maintenance_level, -0.1);
        }

        {
            println!("\ntest_village_update_house_maintenance 2");
            let mut village = Village::new(0, (2, 0), (1, 0), 2, 1);
            village.wood = 0.0;

            let allocation = Allocation {
                wood: 2.0,
                food: 0.0,
                house_construction: 0.0,
            };

            village.update(allocation);

            // House maintenance should stay at 0.0
            assert_eq!(village.houses[0].maintenance_level, 0.0);
        }

        {
            println!("\ntest_village_update_house_maintenance 3");
            let mut village = Village::new(0, (2, 0), (1, 0), 2, 1);
            village.wood = 0.0;
            village.houses[0].maintenance_level = -2.0;

            let allocation = Allocation {
                wood: 2.0,
                food: 0.0,
                house_construction: 0.0,
            };

            village.update(allocation);

            // House maintenance should increase
            assert_eq!(village.houses[0].maintenance_level, -1.9);
        }
    }

    #[test]
    fn test_village_update_worker_productivity() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 0);
        village.workers[0].days_without_food = 1;
        village.workers[0].days_without_shelter = 1;

        let allocation = Allocation {
            wood: 0.6,
            food: 0.0,
            house_construction: 0.0,
        };

        village.update(allocation);

        // Worker productivity should be 0.6 (1.0 - 0.2 - 0.2)
        // This affects production rates
        assert_eq!(village.wood, 100.06); // 0.1 wood * 0.6 productivity
        assert_eq!(village.food, 99.); // 100 - 1.0 consumed + 0 produced
    }

    #[test]
    fn test_village_update_worker_no_shelter() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 0);

        for day in 1..=31 {
            let allocation = Allocation {
                wood: village.worker_days(),
                food: 0.0,
                house_construction: 0.0,
            };

            village.update(allocation);
            println!("Day {}", day);
        }

        assert_eq!(village.workers.len(), 0);
    }

    #[test]
    fn test_village_update_worker_starvation() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 1);
        village.food = 0.0;

        for _ in 0..11 {
            let allocation = Allocation {
                wood: village.worker_days(),
                food: 0.0,
                house_construction: 0.0,
            };

            village.update(allocation);
        }

        assert_eq!(village.workers.len(), 0);
    }

    #[test]
    fn test_village_update_worker_growth() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 1);
        village.workers[0].days_with_both = 101;

        for _ in 0..100 {
            let allocation = Allocation {
                wood: 0.0,
                food: village.worker_days(),
                house_construction: 0.0,
            };

            village.update(allocation);
        }

        assert_eq!(village.workers.len(), 2);
    }

    #[test]
    fn test_village_update_growth_chance() {
        let mut village = Village::new(0, (1, 0), (1, 0), 1, 1);
        village.workers[0].days_with_both = 101;

        let allocation = Allocation {
            wood: 1.0,
            food: 0.0,
            house_construction: 0.0,
        };

        village.update(allocation);

        // Growth chance should be 0.05 after 100 days with both
        assert_eq!(village.workers[0].growth_chance(), 0.05);
    }

    #[test]
    fn test_house_construction_basic() {
        let mut village = Village::new(0, (0, 0), (0, 0), 65, 0);
        village.food = 1000.0;
        village.wood = 20.0;

        // Set worker productivity to match our allocation
        for worker in &mut village.workers {
            // Make sure worker has high productivity
            worker.days_without_food = 0;
            worker.days_without_shelter = 0;
        }

        // Allocate worker days to construction matching the total worker days
        let worker_days = village.worker_days();
        let allocation = Allocation {
            wood: 0.0,
            food: 0.0,
            house_construction: worker_days,
        };

        village.update(allocation);

        // Should have built one house using 10 wood
        assert_eq!(village.houses.len(), 1);
        assert_eq!(village.wood, 9.9);
        assert_eq!(village.construction_progress, worker_days - 60.0);
    }

    #[test]
    fn test_house_construction_partial() {
        let mut village = Village::new(0, (0, 0), (0, 0), 65, 0);
        village.wood = 20.0;

        // Set worker productivity
        let worker_days = village.worker_days();
        let half_worker_days = worker_days / 2.0;

        // First allocate half of worker days
        let allocation1 = Allocation {
            wood: 0.0,
            food: half_worker_days,
            house_construction: half_worker_days,
        };

        village.update(allocation1);

        // Should have accumulated progress but not built a house yet
        assert_eq!(village.houses.len(), 0);
        assert_eq!(village.wood, 20.0);
        assert_eq!(village.construction_progress, half_worker_days);

        // Now allocate remaining worker days
        let worker_days = village.worker_days();
        let half_worker_days = worker_days / 2.0;
        let allocation2 = Allocation {
            wood: 0.0,
            food: 0.0,
            house_construction: village.worker_days(),
        };

        village.update(allocation2);

        println!("Construction progress: {}", village.construction_progress);
        // Should have built one house
        assert_eq!(village.houses.len(), 1);
        assert_eq!(village.wood, 9.9);
        assert!(village.construction_progress < half_worker_days);
    }

    #[test]
    fn test_house_construction_insufficient_wood() {
        let mut village = Village::new(0, (0, 0), (0, 0), 4, 0);
        village.wood = 5.0; // Not enough wood for a house

        // Allocate all worker days
        let worker_days = village.worker_days();
        let allocation = Allocation {
            wood: 0.0,
            food: 0.0,
            house_construction: worker_days,
        };

        village.update(allocation);

        // Should have accumulated progress but not built a house
        assert_eq!(village.houses.len(), 0);
        assert_eq!(village.wood, 5.0);
        assert_eq!(village.construction_progress, worker_days);
    }

    #[test]
    fn test_house_construction_multiple() {
        let mut village = Village::new(0, (0, 0), (0, 0), 130, 0);
        village.wood = 25.0; // Enough for 2 houses

        // Allocate worker days (need more than 120 to build 2 houses)
        let worker_days = village.worker_days();
        let allocation = Allocation {
            wood: 0.0,
            food: 0.0,
            house_construction: worker_days,
        };

        village.update(allocation);

        // Should have built 2 houses
        assert_eq!(village.houses.len(), 2);
        assert_eq!(village.wood.round(), 5.0);
        assert_eq!(village.construction_progress, worker_days - 120.0);
    }

    #[test]
    fn test_house_construction_with_other_allocations() {
        let mut village = Village::new(0, (1, 0), (1, 0), 3, 0);
        village.wood = 15.0;

        // Get total worker days
        let worker_days = village.worker_days();

        // Allocate to multiple tasks, ensuring the total adds up to worker_days
        let allocation = Allocation {
            wood: worker_days / 3.0,
            food: worker_days / 3.0,
            house_construction: worker_days / 3.0,
        };

        village.update(allocation);

        // Should have produced resources and made some construction progress
        assert_eq!(village.construction_progress, worker_days / 3.0);
        // Wood production (0.1) minus no maintenance + no house building
        assert!(village.wood > 15.0);
        // Food production (2.0) minus 3 consumption
        assert!(village.food < 100.0);
    }
}
