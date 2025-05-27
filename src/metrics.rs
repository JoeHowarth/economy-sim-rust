use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::events::{DeathCause, Event, EventType, ResourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageMetrics {
    pub village_id: String,
    pub survival_score: f64,
    pub growth_score: f64,
    pub economic_efficiency: f64,
    pub trade_effectiveness: f64,
    pub stability_score: f64,
    pub overall_score: f64,

    pub initial_population: usize,
    pub final_population: usize,
    pub peak_population: usize,
    pub total_births: usize,
    pub total_deaths: usize,
    pub starvation_deaths: usize,
    pub shelter_deaths: usize,

    pub total_food_produced: Decimal,
    pub total_wood_produced: Decimal,
    pub total_food_consumed: Decimal,
    pub total_wood_consumed: Decimal,

    pub houses_built: usize,
    pub final_houses: usize,
    pub average_house_maintenance: Decimal,

    pub trades_executed: usize,
    pub trade_volume: Decimal,
    pub trade_profit: Decimal,

    pub days_survived: usize,
    pub population_variance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMetrics {
    pub total_days: usize,
    pub villages: HashMap<String, VillageMetrics>,
    pub aggregate_survival_rate: f64,
    pub aggregate_growth_rate: f64,
    pub total_trade_volume: Decimal,
    pub economic_inequality: f64,
}

pub struct MetricsCalculator;

impl MetricsCalculator {
    pub fn calculate_village_metrics(
        village_id: &str,
        events: &[Event],
        initial_population: usize,
        days_simulated: usize,
    ) -> VillageMetrics {
        let village_events: Vec<&Event> = events
            .iter()
            .filter(|e| e.village_id == village_id)
            .collect();

        let mut metrics = VillageMetrics {
            village_id: village_id.to_string(),
            initial_population,
            survival_score: 0.0,
            growth_score: 0.0,
            economic_efficiency: 0.0,
            trade_effectiveness: 0.0,
            stability_score: 0.0,
            overall_score: 0.0,
            final_population: 0,
            peak_population: initial_population,
            total_births: 0,
            total_deaths: 0,
            starvation_deaths: 0,
            shelter_deaths: 0,
            total_food_produced: Decimal::ZERO,
            total_wood_produced: Decimal::ZERO,
            total_food_consumed: Decimal::ZERO,
            total_wood_consumed: Decimal::ZERO,
            houses_built: 0,
            final_houses: 0,
            average_house_maintenance: Decimal::ZERO,
            trades_executed: 0,
            trade_volume: Decimal::ZERO,
            trade_profit: Decimal::ZERO,
            days_survived: days_simulated,
            population_variance: 0.0,
        };

        let mut population_history = vec![initial_population];
        let mut money_history = Vec::new();
        let mut house_maintenance_sum = Decimal::ZERO;
        let mut house_maintenance_count = 0;

        for event in &village_events {
            match &event.event_type {
                EventType::WorkerBorn {
                    total_population, ..
                } => {
                    metrics.total_births += 1;
                    if *total_population > metrics.peak_population {
                        metrics.peak_population = *total_population;
                    }
                    population_history.push(*total_population);
                }
                EventType::WorkerDied {
                    cause,
                    total_population,
                    ..
                } => {
                    metrics.total_deaths += 1;
                    match cause {
                        DeathCause::Starvation => metrics.starvation_deaths += 1,
                        DeathCause::NoShelter => metrics.shelter_deaths += 1,
                    }
                    population_history.push(*total_population);
                }
                EventType::ResourceProduced {
                    resource, amount, ..
                } => match resource {
                    ResourceType::Food => metrics.total_food_produced += amount,
                    ResourceType::Wood => metrics.total_wood_produced += amount,
                },
                EventType::ResourceConsumed {
                    resource, amount, ..
                } => match resource {
                    ResourceType::Food => metrics.total_food_consumed += amount,
                    ResourceType::Wood => metrics.total_wood_consumed += amount,
                },
                EventType::HouseCompleted { total_houses, .. } => {
                    metrics.houses_built += 1;
                    metrics.final_houses = *total_houses;
                }
                EventType::HouseDecayed {
                    maintenance_level, ..
                } => {
                    house_maintenance_sum += maintenance_level;
                    house_maintenance_count += 1;
                }
                EventType::TradeExecuted {
                    quantity,
                    price,
                    side,
                    ..
                } => {
                    metrics.trades_executed += 1;
                    metrics.trade_volume += quantity;
                    let trade_value = quantity * price;
                    match side {
                        crate::events::TradeSide::Sell => metrics.trade_profit += trade_value,
                        crate::events::TradeSide::Buy => metrics.trade_profit -= trade_value,
                    }
                }
                EventType::VillageStateSnapshot {
                    population,
                    houses,
                    money,
                    ..
                } => {
                    metrics.final_population = *population;
                    metrics.final_houses = *houses;
                    money_history.push(*money);
                    if *population == 0 {
                        metrics.days_survived = event.tick;
                    }
                }
                _ => {}
            }
        }

        if house_maintenance_count > 0 {
            metrics.average_house_maintenance =
                house_maintenance_sum / Decimal::from(house_maintenance_count);
        }

        metrics.survival_score = if initial_population > 0 {
            (metrics.final_population as f64 / initial_population as f64).min(1.0)
        } else {
            0.0
        };

        metrics.growth_score = if initial_population > 0 {
            ((metrics.peak_population as f64 - initial_population as f64)
                / initial_population as f64)
                .max(0.0)
        } else {
            0.0
        };

        let avg_population =
            population_history.iter().sum::<usize>() as f64 / population_history.len() as f64;
        if avg_population > 0.0 {
            let total_production = metrics.total_food_produced + metrics.total_wood_produced;
            metrics.economic_efficiency =
                total_production.to_f64().unwrap_or(0.0) / (avg_population * days_simulated as f64);
        }

        if metrics.trades_executed > 0 {
            metrics.trade_effectiveness =
                metrics.trade_profit.to_f64().unwrap_or(0.0) / metrics.trades_executed as f64;
        }

        if !population_history.is_empty() && avg_population > 0.0 {
            let variance = population_history
                .iter()
                .map(|&p| (p as f64 - avg_population).powi(2))
                .sum::<f64>()
                / population_history.len() as f64;
            metrics.population_variance = variance.sqrt();
            metrics.stability_score = 1.0 / (1.0 + metrics.population_variance / avg_population);
        }

        // Overall score is the growth multiplier (final_population / initial_population)
        metrics.overall_score = if initial_population > 0 {
            metrics.final_population as f64 / initial_population as f64
        } else if metrics.final_population > 0 {
            // If started with 0 but have population now, that's infinite growth - cap at 10x
            10.0
        } else {
            0.0
        };

        metrics
    }

    pub fn calculate_scenario_metrics(
        events: &[Event],
        village_configs: &[(String, usize)], // (village_id, initial_population)
        days_simulated: usize,
    ) -> ScenarioMetrics {
        let mut villages = HashMap::new();
        let mut total_initial_pop = 0;
        let mut total_final_pop = 0;

        for (village_id, initial_pop) in village_configs {
            let metrics =
                Self::calculate_village_metrics(village_id, events, *initial_pop, days_simulated);
            total_initial_pop += initial_pop;
            total_final_pop += metrics.final_population;
            villages.insert(village_id.clone(), metrics);
        }

        let aggregate_survival_rate = if total_initial_pop > 0 {
            total_final_pop as f64 / total_initial_pop as f64
        } else {
            0.0
        };

        let total_peak_pop: usize = villages.values().map(|v| v.peak_population).sum();
        let aggregate_growth_rate = if total_initial_pop > 0 {
            (total_peak_pop as f64 - total_initial_pop as f64) / total_initial_pop as f64
        } else {
            0.0
        };

        let total_trade_volume = villages.values().map(|v| v.trade_volume).sum();

        let final_populations: Vec<f64> = villages
            .values()
            .map(|v| v.final_population as f64)
            .collect();
        let economic_inequality = if !final_populations.is_empty() {
            Self::calculate_gini_coefficient(&final_populations)
        } else {
            0.0
        };

        ScenarioMetrics {
            total_days: days_simulated,
            villages,
            aggregate_survival_rate,
            aggregate_growth_rate,
            total_trade_volume,
            economic_inequality,
        }
    }

    pub fn calculate_gini_coefficient(values: &[f64]) -> f64 {
        if values.is_empty() || values.iter().all(|&v| v == 0.0) {
            return 0.0;
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = sorted.len() as f64;
        let total: f64 = sorted.iter().sum();

        if total == 0.0 {
            return 0.0;
        }

        let mut sum = 0.0;

        for (i, &value) in sorted.iter().enumerate() {
            sum += (i as f64 + 1.0) * value;
        }

        2.0 * sum / (n * total) - (n + 1.0) / n
    }
}

impl std::fmt::Display for VillageMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Village {} Metrics:", self.village_id)?;
        writeln!(f, "  Overall Score: {:.2}x", self.overall_score)?;
        writeln!(
            f,
            "  - Survival: {:.2} ({}→{} pop)",
            self.survival_score,
            self.initial_population,
            self.final_population
        )?;
        writeln!(
            f,
            "  - Growth: {:.2} (peak {})",
            self.growth_score, self.peak_population
        )?;
        writeln!(
            f,
            "  - Efficiency: {:.2} res/worker/day",
            self.economic_efficiency
        )?;
        writeln!(f, "  - Trade: {:.2} profit/trade", self.trade_effectiveness)?;
        writeln!(
            f,
            "  - Stability: {:.2} (σ={:.1})",
            self.stability_score, self.population_variance
        )?;
        Ok(())
    }
}

impl std::fmt::Display for ScenarioMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Scenario Metrics ({} days):", self.total_days)?;
        writeln!(
            f,
            "  Aggregate Survival Rate: {:.1}%",
            self.aggregate_survival_rate * 100.0
        )?;
        writeln!(
            f,
            "  Aggregate Growth Rate: {:.1}%",
            self.aggregate_growth_rate * 100.0
        )?;
        writeln!(f, "  Total Trade Volume: {}", self.total_trade_volume)?;
        writeln!(
            f,
            "  Economic Inequality (Gini): {:.3}",
            self.economic_inequality
        )?;
        writeln!(f, "\nVillage Scores (Growth Multiplier):")?;
        let mut sorted_villages: Vec<_> = self.villages.iter().collect();
        sorted_villages.sort_by(|a, b| b.1.overall_score.partial_cmp(&a.1.overall_score).unwrap());
        for (id, metrics) in sorted_villages {
            writeln!(f, "  {}: {:.2}x", id, metrics.overall_score)?;
        }
        Ok(())
    }
}
