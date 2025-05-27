//! Batch experiment runner for systematic strategy evaluation.

use crate::cli::CliArgs;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

/// Configuration for a batch of experiments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentBatch {
    pub name: String,
    pub description: String,
    pub parallel: Option<usize>,
    pub experiments: Vec<ExperimentConfig>,
}

/// Configuration for a single experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub name: String,
    pub scenario: PathBuf,
    pub strategies: Vec<String>,
    pub output: PathBuf,
    #[serde(default)]
    pub overrides: ExperimentOverrides,
    #[serde(default)]
    pub repeat: usize,
}

/// Parameter overrides for an experiment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExperimentOverrides {
    pub days: Option<usize>,
    pub growth_delay: Option<usize>,
    pub random_seed: Option<u64>,
    pub initial_food: Option<Decimal>,
    pub initial_wood: Option<Decimal>,
    pub initial_money: Option<Decimal>,
}

/// Result of running an experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
    pub metrics: Option<ExperimentMetrics>,
    pub duration_ms: u64,
}

/// Summary metrics from an experiment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentMetrics {
    pub aggregate_survival_rate: f64,
    pub aggregate_growth_rate: f64,
    pub total_trade_volume: usize,
    pub economic_inequality: f64,
    pub village_scores: HashMap<String, f64>,
}

impl ExperimentBatch {
    /// Load experiment configuration from YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read experiment file: {}", e))?;

        serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse YAML: {}", e))
    }

    /// Run all experiments in the batch
    pub fn run(&self, quiet: bool) -> Vec<ExperimentResult> {
        let parallel = self.parallel.unwrap_or(1);

        if parallel == 1 {
            // Sequential execution
            self.experiments
                .iter()
                .map(|exp| run_single_experiment(exp, quiet))
                .collect()
        } else {
            // Parallel execution
            let results = Arc::new(Mutex::new(Vec::new()));
            let mut handles = vec![];

            // Create thread pool
            let semaphore = Arc::new(Mutex::new(parallel));

            for exp in &self.experiments {
                let exp_clone = exp.clone();
                let results_clone = Arc::clone(&results);
                let sem_clone = Arc::clone(&semaphore);

                let handle = thread::spawn(move || {
                    // Wait for available slot
                    loop {
                        let mut sem = sem_clone.lock().unwrap();
                        if *sem > 0 {
                            *sem -= 1;
                            break;
                        }
                        drop(sem);
                        thread::sleep(std::time::Duration::from_millis(100));
                    }

                    // Run experiment
                    let result = run_single_experiment(&exp_clone, quiet);

                    // Store result
                    results_clone.lock().unwrap().push(result);

                    // Release slot
                    *sem_clone.lock().unwrap() += 1;
                });

                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }

            Arc::try_unwrap(results).unwrap().into_inner().unwrap()
        }
    }
}

/// Run a single experiment
fn run_single_experiment(config: &ExperimentConfig, quiet: bool) -> ExperimentResult {
    let start = std::time::Instant::now();

    // Prepare CLI args
    let mut args = CliArgs {
        scenario_file: Some(config.scenario.clone()),
        strategies: config.strategies.clone(),
        output_file: Some(config.output.clone()),
        ..Default::default()
    };

    // Apply overrides
    if let Some(days) = config.overrides.days {
        args.days = Some(days);
    }
    if let Some(delay) = config.overrides.growth_delay {
        args.growth_delay = Some(delay);
    }
    if let Some(seed) = config.overrides.random_seed {
        args.random_seed = Some(seed);
    }
    if let Some(food) = config.overrides.initial_food {
        args.initial_food = Some(food);
    }
    if let Some(wood) = config.overrides.initial_wood {
        args.initial_wood = Some(wood);
    }
    if let Some(money) = config.overrides.initial_money {
        args.initial_money = Some(money);
    }

    if !quiet {
        println!("Running experiment: {}", config.name);
    }

    // Run the simulation
    match run_simulation_for_experiment(args, quiet) {
        Ok(metrics) => ExperimentResult {
            name: config.name.clone(),
            success: true,
            error: None,
            metrics: Some(metrics),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => ExperimentResult {
            name: config.name.clone(),
            success: false,
            error: Some(e),
            metrics: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Run simulation and extract metrics (wrapper around main simulation)
fn run_simulation_for_experiment(args: CliArgs, quiet: bool) -> Result<ExperimentMetrics, String> {
    // For now, we'll run the simulation as a subprocess
    // In the future, this should be refactored to call run_simulation directly

    use std::process::Command;

    // Build command
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.arg("run");

    if let Some(ref file) = args.scenario_file {
        cmd.arg("--scenario-file").arg(file);
    }

    for strategy in &args.strategies {
        cmd.arg("-s").arg(strategy);
    }

    if let Some(ref output) = args.output_file {
        cmd.arg("-o").arg(output);
    }

    if let Some(days) = args.days {
        cmd.arg("--days").arg(days.to_string());
    }

    if let Some(delay) = args.growth_delay {
        cmd.arg("--growth-delay").arg(delay.to_string());
    }

    if let Some(seed) = args.random_seed {
        cmd.arg("--seed").arg(seed.to_string());
    }

    if quiet {
        cmd.arg("--quiet");
    }

    // Run simulation
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run simulation: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Simulation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Parse output to extract metrics
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut metrics = ExperimentMetrics {
        aggregate_survival_rate: 0.0,
        aggregate_growth_rate: 0.0,
        total_trade_volume: 0,
        economic_inequality: 0.0,
        village_scores: HashMap::new(),
    };

    // Parse metrics from output
    for line in stdout.lines() {
        if line.contains("Aggregate Survival Rate:") {
            if let Some(value) = extract_percentage(line) {
                metrics.aggregate_survival_rate = value / 100.0;
            }
        } else if line.contains("Aggregate Growth Rate:") {
            if let Some(value) = extract_percentage(line) {
                metrics.aggregate_growth_rate = value / 100.0;
            }
        } else if line.contains("Total Trade Volume:") {
            if let Some(value) = extract_number(line) {
                metrics.total_trade_volume = value as usize;
            }
        } else if line.contains("Economic Inequality (Gini):") {
            if let Some(value) = extract_decimal(line) {
                metrics.economic_inequality = value;
            }
        } else if line.contains("x") && line.contains(":") {
            // Parse village scores like "food_specialist: 2.73x"
            if let Some((village, score)) = parse_village_score(line) {
                metrics.village_scores.insert(village, score);
            }
        }
    }

    Ok(metrics)
}

fn extract_percentage(line: &str) -> Option<f64> {
    // Extract percentage from lines like "Aggregate Survival Rate: 142.5%"
    line.split(':')
        .nth(1)?
        .trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
}

fn extract_number(line: &str) -> Option<f64> {
    // Extract number from lines like "Total Trade Volume: 1870"
    line.split(':').nth(1)?.trim().parse::<f64>().ok()
}

fn extract_decimal(line: &str) -> Option<f64> {
    // Extract decimal from lines like "Economic Inequality (Gini): 0.620"
    line.split(':').nth(1)?.trim().parse::<f64>().ok()
}

fn parse_village_score(line: &str) -> Option<(String, f64)> {
    // Parse lines like "food_specialist: 2.73x"
    let parts: Vec<&str> = line.trim().split(':').collect();
    if parts.len() == 2 {
        let village = parts[0].trim().to_string();
        let score = parts[1].trim().trim_end_matches('x').parse::<f64>().ok()?;
        Some((village, score))
    } else {
        None
    }
}

