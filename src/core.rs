use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[derive(Default, Clone)]
pub struct Worker {
    pub id: usize,
    pub days_without_food: u32,
    pub days_without_shelter: u32,
    pub days_with_both: u32,
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

#[derive(Debug)]
pub struct Allocation {
    pub wood: Decimal,
    pub food: Decimal,
    pub house_construction: Decimal,
}

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

    // For tracking births/deaths
    pub next_worker_id: usize,
    pub next_house_id: usize,
}

impl Village {
    pub fn worker_days(&self) -> Decimal {
        self.workers.iter().map(|w| w.productivity()).sum()
    }
}

pub trait Strategy {
    fn decide_allocation_and_bids_asks(
        &self,
        village: &Village,
        _bids: &[(Decimal, u32, usize)],
        _asks: &[(Decimal, u32, usize)],
    ) -> (Allocation, (Decimal, u32), (Decimal, u32));
}
