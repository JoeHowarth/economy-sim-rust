use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: DateTime<Utc>,
    pub tick: usize,
    pub village_id: String,
    pub event_type: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventType {
    ResourceProduced {
        resource: ResourceType,
        amount: Decimal,
        workers_assigned: usize,
    },
    ResourceConsumed {
        resource: ResourceType,
        amount: Decimal,
        purpose: ConsumptionPurpose,
    },
    WorkerBorn {
        worker_id: usize,
        total_population: usize,
    },
    WorkerDied {
        worker_id: usize,
        cause: DeathCause,
        total_population: usize,
    },
    HouseCompleted {
        house_id: usize,
        total_houses: usize,
    },
    HouseDecayed {
        house_id: usize,
        maintenance_level: Decimal,
    },
    TradeExecuted {
        resource: ResourceType,
        quantity: Decimal,
        price: Decimal,
        counterparty: String,
        side: TradeSide,
    },
    OrderPlaced {
        resource: ResourceType,
        quantity: Decimal,
        price: Decimal,
        side: TradeSide,
        order_id: String,
    },
    WorkerAllocation {
        food_workers: usize,
        wood_workers: usize,
        construction_workers: usize,
        repair_workers: usize,
        idle_workers: usize,
    },
    VillageStateSnapshot {
        population: usize,
        houses: usize,
        food: Decimal,
        wood: Decimal,
        money: Decimal,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Food,
    Wood,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsumptionPurpose {
    WorkerFeeding,
    HouseConstruction,
    HouseMaintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    NoShelter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] Village {}: ", self.tick, self.village_id)?;

        match &self.event_type {
            EventType::ResourceProduced {
                resource,
                amount,
                workers_assigned,
            } => {
                write!(
                    f,
                    "Produced {} {:?} with {} workers",
                    amount, resource, workers_assigned
                )
            }
            EventType::ResourceConsumed {
                resource,
                amount,
                purpose,
            } => {
                write!(f, "Consumed {} {:?} for {:?}", amount, resource, purpose)
            }
            EventType::WorkerBorn {
                worker_id,
                total_population,
            } => {
                write!(
                    f,
                    "Worker {} born (population: {})",
                    worker_id, total_population
                )
            }
            EventType::WorkerDied {
                worker_id,
                cause,
                total_population,
            } => {
                write!(
                    f,
                    "Worker {} died of {:?} (population: {})",
                    worker_id, cause, total_population
                )
            }
            EventType::HouseCompleted {
                house_id,
                total_houses,
            } => {
                write!(f, "House {} completed (total: {})", house_id, total_houses)
            }
            EventType::HouseDecayed {
                house_id,
                maintenance_level,
            } => {
                write!(
                    f,
                    "House {} decayed to {} maintenance",
                    house_id, maintenance_level
                )
            }
            EventType::TradeExecuted {
                resource,
                quantity,
                price,
                counterparty,
                side,
            } => {
                write!(
                    f,
                    "{:?} {} {:?} at {} with {}",
                    side, quantity, resource, price, counterparty
                )
            }
            EventType::OrderPlaced {
                resource,
                quantity,
                price,
                side,
                ..
            } => {
                write!(
                    f,
                    "Placed {:?} order for {} {:?} at {}",
                    side, quantity, resource, price
                )
            }
            EventType::WorkerAllocation {
                food_workers,
                wood_workers,
                construction_workers,
                repair_workers,
                idle_workers,
            } => {
                write!(
                    f,
                    "Allocated workers - F:{} W:{} C:{} R:{} I:{}",
                    food_workers, wood_workers, construction_workers, repair_workers, idle_workers
                )
            }
            EventType::VillageStateSnapshot {
                population,
                houses,
                food,
                wood,
                money,
            } => {
                write!(
                    f,
                    "State - Pop:{} Houses:{} Food:{} Wood:{} Money:{}",
                    population, houses, food, wood, money
                )
            }
        }
    }
}

#[derive(Default)]
pub struct EventLogger {
    events: Vec<Event>,
}

impl EventLogger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log(&mut self, tick: usize, village_id: String, event_type: EventType) {
        self.events.push(Event {
            timestamp: Utc::now(),
            tick,
            village_id,
            event_type,
        });
    }

    pub fn get_events(&self) -> &[Event] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&self.events)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let events: Vec<Event> = serde_json::from_str(&json)?;
        Ok(Self { events })
    }
}
