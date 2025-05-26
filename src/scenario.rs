use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub parameters: SimulationParameters,
    pub villages: Vec<VillageConfig>,
    pub random_seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParameters {
    pub days_to_simulate: usize,
    pub days_without_food_before_starvation: usize,
    pub days_without_shelter_before_death: usize,
    pub days_before_growth_chance: usize,
    pub growth_chance_per_day: f64,
    pub house_construction_days: usize,
    pub house_construction_wood: Decimal,
    pub house_capacity: usize,
    pub house_decay_rate: Decimal,
    pub base_food_production: Decimal,
    pub base_wood_production: Decimal,
    pub second_slot_productivity: f64,
}

impl Default for SimulationParameters {
    fn default() -> Self {
        Self {
            days_to_simulate: 100,
            days_without_food_before_starvation: 10,
            days_without_shelter_before_death: 30,
            days_before_growth_chance: 100,
            growth_chance_per_day: 0.05,
            house_construction_days: 60,
            house_construction_wood: Decimal::from(10),
            house_capacity: 5,
            house_decay_rate: Decimal::from(1),
            base_food_production: Decimal::from(1),
            base_wood_production: Decimal::from(1),
            second_slot_productivity: 0.75,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageConfig {
    pub id: String,
    pub initial_workers: usize,
    pub initial_houses: usize,
    pub initial_food: Decimal,
    pub initial_wood: Decimal,
    pub initial_money: Decimal,
    pub food_slots: (usize, usize),
    pub wood_slots: (usize, usize),
    pub strategy: StrategyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StrategyConfig {
    Balanced {
        food_weight: f64,
        wood_weight: f64,
        construction_weight: f64,
        repair_weight: f64,
    },
    Survival {
        min_food_days: usize,
        min_shelter_buffer: usize,
    },
    Growth {
        target_population: usize,
        house_buffer: usize,
    },
    Trading {
        price_multiplier: f64,
        max_trade_fraction: f64,
    },
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self::Balanced {
            food_weight: 0.25,
            wood_weight: 0.25,
            construction_weight: 0.25,
            repair_weight: 0.25,
        }
    }
}

impl Scenario {
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: String::new(),
            parameters: SimulationParameters::default(),
            villages: Vec::new(),
            random_seed: None,
        }
    }

    pub fn add_village(&mut self, config: VillageConfig) {
        self.villages.push(config);
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let scenario: Self = serde_json::from_str(&json)?;
        Ok(scenario)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.villages.is_empty() {
            return Err("Scenario must have at least one village".to_string());
        }

        for village in &self.villages {
            if village.initial_workers == 0 {
                return Err(format!(
                    "Village {} must have at least one worker",
                    village.id
                ));
            }
            if village.food_slots.0 == 0 || village.wood_slots.0 == 0 {
                return Err(format!(
                    "Village {} must have at least one slot for food and wood",
                    village.id
                ));
            }
        }

        Ok(())
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Scenario: {}", self.name)?;
        writeln!(f, "Description: {}", self.description)?;
        writeln!(f, "\nParameters:")?;
        writeln!(
            f,
            "  Days to simulate: {}",
            self.parameters.days_to_simulate
        )?;
        writeln!(
            f,
            "  Starvation after: {} days",
            self.parameters.days_without_food_before_starvation
        )?;
        writeln!(
            f,
            "  Death without shelter after: {} days",
            self.parameters.days_without_shelter_before_death
        )?;
        writeln!(
            f,
            "  Growth starts after: {} days",
            self.parameters.days_before_growth_chance
        )?;
        writeln!(
            f,
            "  Growth chance: {}%",
            self.parameters.growth_chance_per_day * 100.0
        )?;
        writeln!(
            f,
            "  House construction: {} wood, {} days",
            self.parameters.house_construction_wood, self.parameters.house_construction_days
        )?;

        writeln!(f, "\nVillages:")?;
        for village in &self.villages {
            writeln!(f, "\n  Village: {}", village.id)?;
            writeln!(f, "    Initial population: {}", village.initial_workers)?;
            writeln!(f, "    Initial houses: {}", village.initial_houses)?;
            writeln!(
                f,
                "    Initial resources: {} food, {} wood, {} money",
                village.initial_food, village.initial_wood, village.initial_money
            )?;
            writeln!(
                f,
                "    Production slots: {} food, {} wood",
                village.food_slots.0, village.wood_slots.0
            )?;
            writeln!(f, "    Strategy: {:?}", village.strategy)?;
        }

        Ok(())
    }
}

pub fn create_standard_scenarios() -> HashMap<String, Scenario> {
    let mut scenarios = HashMap::new();

    let mut basic = Scenario::new("basic_two_villages".to_string());
    basic.description = "Two villages with balanced strategies".to_string();
    basic.add_village(VillageConfig {
        id: "village_a".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(50),
        initial_wood: Decimal::from(50),
        initial_money: Decimal::from(100),
        food_slots: (10, 10),
        wood_slots: (10, 10),
        strategy: StrategyConfig::default(),
    });
    basic.add_village(VillageConfig {
        id: "village_b".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(50),
        initial_wood: Decimal::from(50),
        initial_money: Decimal::from(100),
        food_slots: (10, 10),
        wood_slots: (10, 10),
        strategy: StrategyConfig::default(),
    });
    scenarios.insert("basic".to_string(), basic);

    // Custom scenario that allows CLI strategy override
    let mut custom = Scenario::new("custom".to_string());
    custom.description = "Villages with CLI-specified strategies".to_string();
    custom.parameters.days_to_simulate = 200;
    custom.add_village(VillageConfig {
        id: "village_1".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(50),
        initial_wood: Decimal::from(50),
        initial_money: Decimal::from(100),
        food_slots: (10, 10),
        wood_slots: (10, 10),
        strategy: StrategyConfig::default(),
    });
    custom.add_village(VillageConfig {
        id: "village_2".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(50),
        initial_wood: Decimal::from(50),
        initial_money: Decimal::from(100),
        food_slots: (10, 10),
        wood_slots: (10, 10),
        strategy: StrategyConfig::default(),
    });
    scenarios.insert("custom".to_string(), custom);

    let mut scarcity = Scenario::new("resource_scarcity".to_string());
    scarcity.description = "Villages with limited production slots".to_string();
    scarcity.add_village(VillageConfig {
        id: "scarce_village".to_string(),
        initial_workers: 15,
        initial_houses: 3,
        initial_food: Decimal::from(30),
        initial_wood: Decimal::from(30),
        initial_money: Decimal::from(50),
        food_slots: (5, 5),
        wood_slots: (5, 5),
        strategy: StrategyConfig::Survival {
            min_food_days: 15,
            min_shelter_buffer: 2,
        },
    });
    scenarios.insert("scarcity".to_string(), scarcity);

    let mut growth = Scenario::new("growth_focused".to_string());
    growth.description = "Villages optimized for population growth".to_string();
    growth.parameters.days_to_simulate = 200;
    growth.add_village(VillageConfig {
        id: "growth_village".to_string(),
        initial_workers: 5,
        initial_houses: 2,
        initial_food: Decimal::from(100),
        initial_wood: Decimal::from(100),
        initial_money: Decimal::from(200),
        food_slots: (20, 20),
        wood_slots: (20, 20),
        strategy: StrategyConfig::Growth {
            target_population: 50,
            house_buffer: 3,
        },
    });
    scenarios.insert("growth".to_string(), growth);

    // Trading scenario with specialized villages
    let mut trading = Scenario::new("trading".to_string());
    trading.description = "Villages specialized for trading".to_string();
    trading.parameters.days_to_simulate = 150;
    trading.add_village(VillageConfig {
        id: "wood_specialist".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(30),
        initial_wood: Decimal::from(80),
        initial_money: Decimal::from(100),
        food_slots: (5, 5),   // Poor food production
        wood_slots: (20, 10), // Excellent wood production
        strategy: StrategyConfig::Trading {
            price_multiplier: 1.0,
            max_trade_fraction: 0.5,
        },
    });
    trading.add_village(VillageConfig {
        id: "food_specialist".to_string(),
        initial_workers: 10,
        initial_houses: 2,
        initial_food: Decimal::from(80),
        initial_wood: Decimal::from(30),
        initial_money: Decimal::from(100),
        food_slots: (20, 10), // Excellent food production
        wood_slots: (5, 5),   // Poor wood production
        strategy: StrategyConfig::Trading {
            price_multiplier: 1.0,
            max_trade_fraction: 0.5,
        },
    });
    scenarios.insert("trading".to_string(), trading);

    scenarios
}
