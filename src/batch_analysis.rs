//! Batch analysis tools for comparing multiple simulation results.

use crate::analysis::analyze_simulation;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Results from batch analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAnalysisReport {
    pub simulations: Vec<SimulationSummary>,
    pub aggregate_stats: AggregateStatistics,
    pub strategy_performance: HashMap<String, StrategyStats>,
    pub insights: Vec<String>,
}

/// Summary of a single simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub file_name: String,
    pub total_days: usize,
    pub villages: Vec<VillageSummary>,
    pub aggregate_survival_rate: f64,
    pub aggregate_growth_rate: f64,
    pub total_trades: usize,
    pub gini_coefficient: f64,
}

/// Summary of a single village
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageSummary {
    pub id: String,
    pub strategy: Option<String>,
    pub growth_multiplier: f64,
    pub final_population: usize,
    pub trade_profit: Decimal,
    pub efficiency: f64,
}

/// Aggregate statistics across all simulations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStatistics {
    pub mean_growth_rate: f64,
    pub std_growth_rate: f64,
    pub mean_survival_rate: f64,
    pub std_survival_rate: f64,
    pub mean_trade_volume: f64,
    pub mean_gini: f64,
    pub total_simulations: usize,
}

/// Performance statistics for a strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub occurrences: usize,
    pub mean_growth: f64,
    pub std_growth: f64,
    pub mean_survival: f64,
    pub mean_efficiency: f64,
    pub total_profit: Decimal,
}

/// Analyze multiple simulation results
pub fn analyze_batch(files: &[PathBuf]) -> Result<BatchAnalysisReport, String> {
    let mut simulations = Vec::new();
    let mut all_growth_rates = Vec::new();
    let mut all_survival_rates = Vec::new();
    let mut all_trade_volumes = Vec::new();
    let mut all_gini_coeffs = Vec::new();
    let mut strategy_data: HashMap<String, Vec<(f64, f64, f64, Decimal)>> = HashMap::new();

    // Analyze each simulation
    for file in files {
        let analysis = analyze_simulation(file)?;

        // Calculate aggregate metrics
        let total_initial_pop: usize = analysis.villages.iter().map(|v| v.initial_population).sum();
        let total_final_pop: usize = analysis.villages.iter().map(|v| v.final_population).sum();

        let aggregate_survival = if total_initial_pop > 0 {
            total_final_pop as f64 / total_initial_pop as f64
        } else {
            0.0
        };

        let aggregate_growth = if total_initial_pop > 0 {
            (total_final_pop as f64 - total_initial_pop as f64) / total_initial_pop as f64
        } else {
            0.0
        };

        all_growth_rates.push(aggregate_growth);
        all_survival_rates.push(aggregate_survival);
        all_trade_volumes.push(analysis.market.total_trades as f64);

        // Extract village summaries
        let mut village_summaries = Vec::new();
        for village in &analysis.villages {
            let growth_multiplier = if village.initial_population > 0 {
                village.final_population as f64 / village.initial_population as f64
            } else {
                0.0
            };

            let efficiency = (village.total_production.food + village.total_production.wood)
                .to_f64()
                .unwrap_or(0.0)
                / village.initial_population.max(1) as f64
                / analysis.total_days as f64;

            // Try to extract strategy from village ID (e.g., "village_1_balanced")
            let strategy = extract_strategy_from_id(&village.id);

            if let Some(ref strat) = strategy {
                strategy_data
                    .entry(strat.clone())
                    .or_default()
                    .push((
                        growth_multiplier,
                        village.survival_rate,
                        efficiency,
                        village.trading_summary.net_profit,
                    ));
            }

            village_summaries.push(VillageSummary {
                id: village.id.clone(),
                strategy,
                growth_multiplier,
                final_population: village.final_population,
                trade_profit: village.trading_summary.net_profit,
                efficiency,
            });
        }

        // Calculate Gini coefficient (placeholder - would need actual wealth distribution)
        let gini = calculate_gini_from_villages(&analysis.villages);
        all_gini_coeffs.push(gini);

        simulations.push(SimulationSummary {
            file_name: file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            total_days: analysis.total_days,
            villages: village_summaries,
            aggregate_survival_rate: aggregate_survival,
            aggregate_growth_rate: aggregate_growth,
            total_trades: analysis.market.total_trades,
            gini_coefficient: gini,
        });
    }

    // Calculate aggregate statistics
    let aggregate_stats = AggregateStatistics {
        mean_growth_rate: mean(&all_growth_rates),
        std_growth_rate: std_dev(&all_growth_rates),
        mean_survival_rate: mean(&all_survival_rates),
        std_survival_rate: std_dev(&all_survival_rates),
        mean_trade_volume: mean(&all_trade_volumes),
        mean_gini: mean(&all_gini_coeffs),
        total_simulations: simulations.len(),
    };

    // Calculate strategy performance
    let mut strategy_performance = HashMap::new();
    for (strategy, data) in strategy_data {
        let growths: Vec<f64> = data.iter().map(|(g, _, _, _)| *g).collect();
        let survivals: Vec<f64> = data.iter().map(|(_, s, _, _)| *s).collect();
        let efficiencies: Vec<f64> = data.iter().map(|(_, _, e, _)| *e).collect();
        let total_profit: Decimal = data.iter().map(|(_, _, _, p)| *p).sum();

        strategy_performance.insert(
            strategy,
            StrategyStats {
                occurrences: data.len(),
                mean_growth: mean(&growths),
                std_growth: std_dev(&growths),
                mean_survival: mean(&survivals),
                mean_efficiency: mean(&efficiencies),
                total_profit,
            },
        );
    }

    // Generate insights
    let insights = generate_batch_insights(&simulations, &aggregate_stats, &strategy_performance);

    Ok(BatchAnalysisReport {
        simulations,
        aggregate_stats,
        strategy_performance,
        insights,
    })
}

/// Export batch analysis to CSV
pub fn export_batch_to_csv(report: &BatchAnalysisReport, output: &Path) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        fs::File::create(output).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write header
    writeln!(
        file,
        "simulation,village_id,strategy,growth_multiplier,final_population,trade_profit,efficiency"
    )
    .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write data
    for sim in &report.simulations {
        for village in &sim.villages {
            writeln!(
                file,
                "\"{}\",\"{}\",\"{}\",{:.3},{},{:.2},{:.3}",
                sim.file_name,
                village.id,
                village.strategy.as_ref().unwrap_or(&"unknown".to_string()),
                village.growth_multiplier,
                village.final_population,
                village.trade_profit,
                village.efficiency
            )
            .map_err(|e| format!("Failed to write CSV row: {}", e))?;
        }
    }

    Ok(())
}

/// Helper functions
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn extract_strategy_from_id(id: &str) -> Option<String> {
    // Try to extract strategy from ID patterns like "village_1_balanced"
    let parts: Vec<&str> = id.split('_').collect();
    if parts.len() >= 3 {
        Some(parts[2..].join("_"))
    } else {
        None
    }
}

fn calculate_gini_from_villages(villages: &[crate::analysis::VillageAnalysis]) -> f64 {
    // Simple Gini calculation based on final populations
    let mut populations: Vec<f64> = villages.iter().map(|v| v.final_population as f64).collect();
    populations.sort_by(|a, b| a.partial_cmp(b).unwrap());

    if populations.is_empty() || populations.iter().all(|&p| p == 0.0) {
        return 0.0;
    }

    let n = populations.len() as f64;
    let sum_of_absolute_differences: f64 = populations
        .iter()
        .enumerate()
        .flat_map(|(i, &xi)| {
            populations
                .iter()
                .skip(i + 1)
                .map(move |&xj| (xi - xj).abs())
        })
        .sum();

    let mean_pop = populations.iter().sum::<f64>() / n;

    if mean_pop == 0.0 {
        return 0.0;
    }

    sum_of_absolute_differences / (n * n * mean_pop)
}

fn generate_batch_insights(
    _simulations: &[SimulationSummary],
    stats: &AggregateStatistics,
    strategies: &HashMap<String, StrategyStats>,
) -> Vec<String> {
    let mut insights = Vec::new();

    // High-level insights
    if stats.std_growth_rate > 0.5 {
        insights.push(format!(
            "High variability in growth rates (σ={:.2}) suggests inconsistent outcomes",
            stats.std_growth_rate
        ));
    }

    if stats.mean_survival_rate < 0.8 {
        insights.push(format!(
            "Low average survival rate ({:.1}%) indicates challenging conditions",
            stats.mean_survival_rate * 100.0
        ));
    }

    // Strategy insights
    if !strategies.is_empty() {
        let best_strategy = strategies
            .iter()
            .max_by(|(_, a), (_, b)| a.mean_growth.partial_cmp(&b.mean_growth).unwrap())
            .map(|(name, _)| name);

        if let Some(best) = best_strategy {
            insights.push(format!(
                "{} strategy performed best with {:.1}% average growth",
                best,
                strategies[best].mean_growth * 100.0
            ));
        }
    }

    // Trade insights
    if stats.mean_trade_volume < 10.0 {
        insights.push("Very low trading activity across simulations".to_string());
    }

    insights
}
