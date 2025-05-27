//! Analysis tools for simulation results.

use crate::events::{Event, EventType, ResourceType, TradeSide};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Analysis results for a simulation.
#[derive(Debug)]
pub struct SimulationAnalysis {
    pub total_events: usize,
    pub total_days: usize,
    pub villages: Vec<VillageAnalysis>,
    pub market: MarketAnalysis,
    pub insights: Vec<String>,
}

#[derive(Debug)]
pub struct VillageAnalysis {
    pub id: String,
    pub initial_population: usize,
    pub final_population: usize,
    pub peak_population: usize,
    pub growth_rate: f64,
    pub survival_rate: f64,
    pub total_production: ResourceProduction,
    pub total_consumption: ResourceProduction,
    pub trading_summary: TradingSummary,
    pub worker_deaths: HashMap<String, usize>, // cause -> count
    pub strategy_effectiveness: f64,
}

#[derive(Debug, Default)]
pub struct ResourceProduction {
    pub food: Decimal,
    pub wood: Decimal,
}

#[derive(Debug, Default)]
pub struct TradingSummary {
    pub total_trades: usize,
    pub buy_orders: usize,
    pub sell_orders: usize,
    pub executed_buys: usize,
    pub executed_sells: usize,
    pub total_spent: Decimal,
    pub total_earned: Decimal,
    pub net_profit: Decimal,
}

#[derive(Debug)]
pub struct MarketAnalysis {
    pub total_orders: usize,
    pub total_trades: usize,
    pub trade_success_rate: f64,
    pub price_history: PriceHistory,
    pub volume_by_resource: HashMap<String, Decimal>,
}

#[derive(Debug, Default)]
pub struct PriceHistory {
    pub wood_prices: Vec<(usize, Decimal)>, // (tick, price)
    pub food_prices: Vec<(usize, Decimal)>,
    pub avg_wood_price: Option<Decimal>,
    pub avg_food_price: Option<Decimal>,
    pub wood_volatility: f64,
    pub food_volatility: f64,
}

/// Load and analyze simulation events from a file.
pub fn analyze_simulation(path: &Path) -> Result<SimulationAnalysis, String> {
    // Load events
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let events: Vec<Event> = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    analyze_events(&events)
}

/// Analyze a set of events.
pub fn analyze_events(events: &[Event]) -> Result<SimulationAnalysis, String> {
    let mut villages: HashMap<String, VillageData> = HashMap::new();
    let mut market_data = MarketData::default();
    let mut max_tick = 0;
    
    // Process each event
    for event in events {
        max_tick = max_tick.max(event.tick);
        
        match &event.event_type {
            EventType::WorkerAllocation { food_workers, wood_workers, .. } => {
                // Track worker allocation patterns
                let village = villages.entry(event.village_id.clone()).or_default();
                village.allocations.push((*food_workers as u32, *wood_workers as u32));
            }
            
            EventType::ResourceProduced { resource, amount, .. } => {
                let village = villages.entry(event.village_id.clone()).or_default();
                match resource {
                    ResourceType::Food => village.total_production.food += amount,
                    ResourceType::Wood => village.total_production.wood += amount,
                }
            }
            
            EventType::ResourceConsumed { resource, amount, .. } => {
                let village = villages.entry(event.village_id.clone()).or_default();
                match resource {
                    ResourceType::Food => village.total_consumption.food += amount,
                    ResourceType::Wood => village.total_consumption.wood += amount,
                }
            }
            
            EventType::VillageStateSnapshot { population, .. } => {
                let village = villages.entry(event.village_id.clone()).or_default();
                village.population_history.push((event.tick, *population));
                if village.initial_population == 0 {
                    village.initial_population = *population;
                }
                village.final_population = *population;
                village.peak_population = village.peak_population.max(*population);
            }
            
            EventType::WorkerDied { cause, .. } => {
                let village = villages.entry(event.village_id.clone()).or_default();
                *village.deaths.entry(format!("{:?}", cause)).or_insert(0) += 1;
            }
            
            EventType::OrderPlaced { side, .. } => {
                market_data.total_orders += 1;
                let village = villages.entry(event.village_id.clone()).or_default();
                match side {
                    TradeSide::Buy => village.trading.buy_orders += 1,
                    TradeSide::Sell => village.trading.sell_orders += 1,
                }
            }
            
            EventType::TradeExecuted { resource, quantity, price, side, .. } => {
                market_data.total_trades += 1;
                let village = villages.entry(event.village_id.clone()).or_default();
                village.trading.total_trades += 1;
                
                let value = price * Decimal::from(*quantity);
                match side {
                    TradeSide::Buy => {
                        village.trading.executed_buys += 1;
                        village.trading.total_spent += value;
                    }
                    TradeSide::Sell => {
                        village.trading.executed_sells += 1;
                        village.trading.total_earned += value;
                    }
                }
                
                // Track market prices
                match resource {
                    ResourceType::Wood => market_data.wood_prices.push((event.tick, *price)),
                    ResourceType::Food => market_data.food_prices.push((event.tick, *price)),
                }
                
                *market_data.volume_by_resource.entry(format!("{:?}", resource)).or_insert(Decimal::ZERO) += Decimal::from(*quantity);
            }
            
            // TODO: Add AuctionCleared event handling when the event type is available
            // EventType::AuctionCleared { clearing_prices } => {
            //     for (resource, price) in clearing_prices {
            //         match resource {
            //             ResourceType::Wood => market_data.wood_prices.push((event.tick, *price)),
            //             ResourceType::Food => market_data.food_prices.push((event.tick, *price)),
            //         }
            //     }
            // }
            
            _ => {}
        }
    }
    
    // Convert to analysis results
    let mut village_analyses = Vec::new();
    for (id, data) in villages {
        let growth_rate = if data.initial_population > 0 {
            (data.final_population as f64 - data.initial_population as f64) / data.initial_population as f64
        } else {
            0.0
        };
        
        let survival_rate = if data.initial_population > 0 {
            data.final_population as f64 / data.initial_population as f64
        } else {
            1.0
        };
        
        let net_profit = data.trading.total_earned - data.trading.total_spent;
        
        let effectiveness = calculate_effectiveness(&data);
        
        village_analyses.push(VillageAnalysis {
            id: id.clone(),
            initial_population: data.initial_population,
            final_population: data.final_population,
            peak_population: data.peak_population,
            growth_rate,
            survival_rate,
            total_production: data.total_production,
            total_consumption: data.total_consumption,
            trading_summary: TradingSummary {
                net_profit,
                ..data.trading
            },
            worker_deaths: data.deaths,
            strategy_effectiveness: effectiveness,
        });
    }
    
    // Calculate market statistics
    let price_history = calculate_price_statistics(&market_data);
    let trade_success_rate = if market_data.total_orders > 0 {
        (market_data.total_trades * 2) as f64 / market_data.total_orders as f64
    } else {
        0.0
    };
    
    // Generate insights
    let insights = generate_insights(&village_analyses, &price_history, max_tick);
    
    Ok(SimulationAnalysis {
        total_events: events.len(),
        total_days: max_tick + 1,
        villages: village_analyses,
        market: MarketAnalysis {
            total_orders: market_data.total_orders,
            total_trades: market_data.total_trades,
            trade_success_rate,
            price_history,
            volume_by_resource: market_data.volume_by_resource,
        },
        insights,
    })
}

/// Compare multiple simulation results.
pub fn compare_simulations(analyses: &[SimulationAnalysis]) -> ComparisonReport {
    let mut report = ComparisonReport::default();
    
    // Compare overall performance
    for (i, analysis) in analyses.iter().enumerate() {
        let avg_growth = analysis.villages.iter()
            .map(|v| v.growth_rate)
            .sum::<f64>() / analysis.villages.len() as f64;
        
        let avg_survival = analysis.villages.iter()
            .map(|v| v.survival_rate)
            .sum::<f64>() / analysis.villages.len() as f64;
        
        let total_trades = analysis.market.total_trades;
        
        report.simulation_summaries.push(SimulationSummary {
            index: i,
            avg_growth_rate: avg_growth,
            avg_survival_rate: avg_survival,
            total_trades,
            trade_success_rate: analysis.market.trade_success_rate,
        });
    }
    
    // Find best performing strategies
    let mut strategy_performance: HashMap<String, Vec<f64>> = HashMap::new();
    for analysis in analyses {
        for village in &analysis.villages {
            if let Some(strategy) = extract_strategy_name(&village.id) {
                strategy_performance.entry(strategy)
                    .or_default()
                    .push(village.strategy_effectiveness);
            }
        }
    }
    
    for (strategy, scores) in strategy_performance {
        let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
        report.strategy_rankings.push((strategy, avg_score));
    }
    
    report.strategy_rankings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    
    report
}

/// Generate a narrative explanation of simulation events.
pub fn explain_simulation(analysis: &SimulationAnalysis) -> String {
    let mut explanation = String::new();
    
    explanation.push_str(&format!("# Simulation Narrative ({} days)\n\n", analysis.total_days));
    
    // Overall summary
    explanation.push_str("## Overview\n\n");
    let total_initial_pop: usize = analysis.villages.iter().map(|v| v.initial_population).sum();
    let total_final_pop: usize = analysis.villages.iter().map(|v| v.final_population).sum();
    
    explanation.push_str(&format!(
        "The simulation began with {} workers across {} villages. \
         After {} days, the total population {} to {} workers ({:+.1}%).\n\n",
        total_initial_pop,
        analysis.villages.len(),
        analysis.total_days,
        if total_final_pop >= total_initial_pop { "grew" } else { "shrank" },
        total_final_pop,
        ((total_final_pop as f64 - total_initial_pop as f64) / total_initial_pop as f64) * 100.0
    ));
    
    // Market activity
    if analysis.market.total_trades > 0 {
        explanation.push_str(&format!(
            "The market saw {} trades out of {} orders placed ({:.1}% success rate). \
             Trading volume was dominated by {:?}.\n\n",
            analysis.market.total_trades,
            analysis.market.total_orders,
            analysis.market.trade_success_rate * 100.0,
            analysis.market.volume_by_resource.iter()
                .max_by_key(|(_, v)| **v)
                .map(|(k, _)| k)
                .unwrap_or(&"none".to_string())
        ));
    } else {
        explanation.push_str("No trading occurred during the simulation.\n\n");
    }
    
    // Village stories
    explanation.push_str("## Village Stories\n\n");
    for village in &analysis.villages {
        explanation.push_str(&format!("### {}\n\n", village.id));
        
        let fate = if village.final_population == 0 {
            "completely died out"
        } else if village.final_population < village.initial_population {
            "struggled to survive"
        } else if village.growth_rate > 0.5 {
            "thrived and grew significantly"
        } else if village.growth_rate > 0.0 {
            "grew modestly"
        } else {
            "maintained its population"
        };
        
        explanation.push_str(&format!(
            "{} {} over the simulation period, going from {} to {} workers.\n",
            village.id, fate, village.initial_population, village.final_population
        ));
        
        // Deaths
        if !village.worker_deaths.is_empty() {
            let total_deaths: usize = village.worker_deaths.values().sum();
            let main_cause = village.worker_deaths.iter()
                .max_by_key(|(_, v)| **v)
                .map(|(k, _)| k.as_str())
                .unwrap_or("unknown");
            
            explanation.push_str(&format!(
                "The village lost {} workers, primarily due to {}.\n",
                total_deaths, main_cause
            ));
        }
        
        // Trading
        if village.trading_summary.total_trades > 0 {
            if village.trading_summary.net_profit > Decimal::ZERO {
                explanation.push_str(&format!(
                    "Through shrewd trading, they earned a profit of {:.2}.\n",
                    village.trading_summary.net_profit
                ));
            } else if village.trading_summary.net_profit < Decimal::ZERO {
                explanation.push_str(&format!(
                    "Trading proved costly, with losses of {:.2}.\n",
                    village.trading_summary.net_profit.abs()
                ));
            }
        }
        
        explanation.push_str("\n");
    }
    
    // Key insights
    if !analysis.insights.is_empty() {
        explanation.push_str("## Key Insights\n\n");
        for insight in &analysis.insights {
            explanation.push_str(&format!("- {}\n", insight));
        }
    }
    
    explanation
}

// Helper structures
#[derive(Default)]
struct VillageData {
    initial_population: usize,
    final_population: usize,
    peak_population: usize,
    population_history: Vec<(usize, usize)>,
    total_production: ResourceProduction,
    total_consumption: ResourceProduction,
    trading: TradingSummary,
    deaths: HashMap<String, usize>,
    allocations: Vec<(u32, u32)>, // (food_workers, wood_workers)
}

#[derive(Default)]
struct MarketData {
    total_orders: usize,
    total_trades: usize,
    wood_prices: Vec<(usize, Decimal)>,
    food_prices: Vec<(usize, Decimal)>,
    volume_by_resource: HashMap<String, Decimal>,
}

#[derive(Debug, Default)]
pub struct ComparisonReport {
    pub simulation_summaries: Vec<SimulationSummary>,
    pub strategy_rankings: Vec<(String, f64)>,
}

#[derive(Debug)]
pub struct SimulationSummary {
    pub index: usize,
    pub avg_growth_rate: f64,
    pub avg_survival_rate: f64,
    pub total_trades: usize,
    pub trade_success_rate: f64,
}

// Helper functions
fn calculate_effectiveness(data: &VillageData) -> f64 {
    let growth_score = if data.initial_population > 0 {
        (data.final_population as f64 / data.initial_population as f64).min(2.0)
    } else {
        0.0
    };
    
    let efficiency_score = if data.final_population > 0 {
        let total_produced = data.total_production.food + data.total_production.wood;
        let avg_pop = (data.initial_population + data.final_population) as f64 / 2.0;
        (total_produced.to_f64().unwrap_or(0.0) / avg_pop / 100.0).min(2.0)
    } else {
        0.0
    };
    
    let trade_score = if data.trading.total_trades > 0 {
        let profit_per_trade = data.trading.net_profit / Decimal::from(data.trading.total_trades);
        (1.0 + profit_per_trade.to_f64().unwrap_or(0.0) / 10.0).max(0.0).min(2.0)
    } else {
        1.0
    };
    
    (growth_score + efficiency_score + trade_score) / 3.0
}

fn calculate_price_statistics(market_data: &MarketData) -> PriceHistory {
    let mut history = PriceHistory::default();
    
    // Wood prices
    if !market_data.wood_prices.is_empty() {
        history.wood_prices = market_data.wood_prices.clone();
        let sum: Decimal = market_data.wood_prices.iter().map(|(_, p)| *p).sum();
        history.avg_wood_price = Some(sum / Decimal::from(market_data.wood_prices.len()));
        
        if market_data.wood_prices.len() > 1 {
            let prices: Vec<f64> = market_data.wood_prices.iter()
                .map(|(_, p)| p.to_f64().unwrap_or(0.0))
                .collect();
            history.wood_volatility = calculate_volatility(&prices);
        }
    }
    
    // Food prices
    if !market_data.food_prices.is_empty() {
        history.food_prices = market_data.food_prices.clone();
        let sum: Decimal = market_data.food_prices.iter().map(|(_, p)| *p).sum();
        history.avg_food_price = Some(sum / Decimal::from(market_data.food_prices.len()));
        
        if market_data.food_prices.len() > 1 {
            let prices: Vec<f64> = market_data.food_prices.iter()
                .map(|(_, p)| p.to_f64().unwrap_or(0.0))
                .collect();
            history.food_volatility = calculate_volatility(&prices);
        }
    }
    
    history
}

fn calculate_volatility(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }
    
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    let variance = prices.iter()
        .map(|p| (p - mean).powi(2))
        .sum::<f64>() / prices.len() as f64;
    
    variance.sqrt() / mean
}

fn generate_insights(villages: &[VillageAnalysis], price_history: &PriceHistory, total_days: usize) -> Vec<String> {
    let mut insights = Vec::new();
    
    // Population insights
    let total_growth: f64 = villages.iter().map(|v| v.growth_rate).sum::<f64>() / villages.len() as f64;
    if total_growth < 0.0 {
        insights.push("Population declined overall - check resource availability and strategy effectiveness".to_string());
    } else if total_growth < 0.1 && total_days >= 100 {
        insights.push("Minimal population growth suggests timing issues - consider adjusting growth delay".to_string());
    }
    
    // Trading insights
    let villages_that_traded = villages.iter().filter(|v| v.trading_summary.total_trades > 0).count();
    if villages_that_traded == 0 {
        insights.push("No trading occurred - strategies may be too similar or prices misaligned".to_string());
    } else if villages_that_traded == 1 {
        insights.push("Only one village participated in trading - market may be one-sided".to_string());
    }
    
    // Price insights
    if price_history.wood_volatility > 0.3 {
        insights.push("High wood price volatility indicates unstable market conditions".to_string());
    }
    if price_history.food_volatility > 0.3 {
        insights.push("High food price volatility indicates unstable market conditions".to_string());
    }
    
    // Death insights
    let total_deaths: usize = villages.iter()
        .flat_map(|v| v.worker_deaths.values())
        .sum();
    if total_deaths > villages.iter().map(|v| v.initial_population).sum::<usize>() / 2 {
        insights.push("High death rate indicates severe resource shortages or strategy failures".to_string());
    }
    
    insights
}

fn extract_strategy_name(village_id: &str) -> Option<String> {
    // This is a placeholder - in reality we'd need to track strategy assignments
    // For now, just return the village ID
    Some(village_id.to_string())
}