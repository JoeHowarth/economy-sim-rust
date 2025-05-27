//! Enhanced command-line interface for the village model simulation.

use crate::scenario::Scenario;
use lexopt::prelude::*;
use rust_decimal::Decimal;
use std::path::PathBuf;

/// Command-line arguments for the simulation.
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub command: Command,
    pub strategies: Vec<String>,
    pub scenario_name: String,
    pub scenario_file: Option<PathBuf>,
    pub days: Option<usize>,
    pub growth_delay: Option<usize>,
    pub random_seed: Option<u64>,
    pub initial_food: Option<Decimal>,
    pub initial_wood: Option<Decimal>,
    pub initial_money: Option<Decimal>,
    pub debug: bool,
    pub verbose: bool,
    pub quiet: bool,
    pub output_file: Option<PathBuf>,
    pub debug_decisions: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Command {
    Run,
    Ui {
        file: PathBuf,
    },
    Analyze {
        file: PathBuf,
    },
    Compare {
        files: Vec<PathBuf>,
    },
    Explain {
        file: PathBuf,
    },
    Batch {
        config: PathBuf,
    },
    AnalyzeBatch {
        files: Vec<PathBuf>,
        output: Option<PathBuf>,
    },
    Query {
        file: PathBuf,
        filters: QueryFilters,
    },
}

#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    pub village: Option<String>,
    pub event_type: Option<String>,
    pub tick_range: Option<(usize, usize)>,
    pub resource: Option<String>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            command: Command::Run,
            strategies: Vec::new(),
            scenario_name: "basic".to_string(),
            scenario_file: None,
            days: None,
            growth_delay: None,
            random_seed: None,
            initial_food: None,
            initial_wood: None,
            initial_money: None,
            debug: false,
            verbose: false,
            quiet: false,
            output_file: None,
            debug_decisions: None,
        }
    }
}

pub fn parse_args() -> Result<CliArgs, lexopt::Error> {
    let mut args = lexopt::Parser::from_env();
    let mut cli_args = CliArgs::default();
    let mut subcommand = None;
    let mut ui_file = None;
    let mut analyze_file = None;
    let mut explain_file = None;
    let mut compare_files = Vec::new();
    let mut batch_config = None;
    let mut analyze_batch_files = Vec::new();
    let mut analyze_batch_output = None;
    let mut query_file = None;
    let mut query_filters = QueryFilters::default();

    while let Some(arg) = args.next()? {
        match arg {
            Value(val) => {
                let val_str = val.string()?;
                if subcommand.is_none() {
                    subcommand = Some(val_str);
                } else {
                    match subcommand.as_deref() {
                        Some("ui") => ui_file = Some(PathBuf::from(val_str)),
                        Some("analyze") => analyze_file = Some(PathBuf::from(val_str)),
                        Some("explain") => explain_file = Some(PathBuf::from(val_str)),
                        Some("compare") => compare_files.push(PathBuf::from(val_str)),
                        Some("batch") => batch_config = Some(PathBuf::from(val_str)),
                        Some("analyze-batch") => analyze_batch_files.push(PathBuf::from(val_str)),
                        Some("query") => query_file = Some(PathBuf::from(val_str)),
                        _ => {}
                    }
                }
            }
            Long("strategy") | Short('s') => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.strategies.push(val.string()?);
                }
            }
            Long("scenario") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.scenario_name = val.string()?;
                }
            }
            Long("scenario-file") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.scenario_file = Some(PathBuf::from(val.string()?));
                }
            }
            Long("days") | Short('d') => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.days = Some(val.parse()?);
                }
            }
            Long("growth-delay") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.growth_delay = Some(val.parse()?);
                }
            }
            Long("seed") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.random_seed = Some(val.parse()?);
                }
            }
            Long("initial-food") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.initial_food = Some(val.parse()?);
                }
            }
            Long("initial-wood") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.initial_wood = Some(val.parse()?);
                }
            }
            Long("initial-money") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.initial_money = Some(val.parse()?);
                }
            }
            Long("debug") => cli_args.debug = true,
            Long("verbose") | Short('v') => cli_args.verbose = true,
            Long("quiet") | Short('q') => cli_args.quiet = true,
            Long("debug-decisions") => {
                if let Some(Value(val)) = args.next()? {
                    cli_args.debug_decisions = Some(val.string()?);
                }
            }
            Long("output") | Short('o') => {
                if let Some(Value(val)) = args.next()? {
                    let path = PathBuf::from(val.string()?);
                    if subcommand.as_deref() == Some("analyze-batch") {
                        analyze_batch_output = Some(path);
                    } else {
                        cli_args.output_file = Some(path);
                    }
                }
            }
            Long("village") => {
                if let Some(Value(val)) = args.next()? {
                    query_filters.village = Some(val.string()?);
                }
            }
            Long("event-type") => {
                if let Some(Value(val)) = args.next()? {
                    query_filters.event_type = Some(val.string()?);
                }
            }
            Long("resource") => {
                if let Some(Value(val)) = args.next()? {
                    query_filters.resource = Some(val.string()?);
                }
            }
            Long("tick-range") => {
                if let Some(Value(val)) = args.next()? {
                    let range_str = val.string()?;
                    if let Some((start, end)) = range_str.split_once('-') {
                        match (start.parse::<usize>(), end.parse::<usize>()) {
                            (Ok(s), Ok(e)) => query_filters.tick_range = Some((s, e)),
                            _ => return Err(lexopt::Error::from("Invalid tick range format")),
                        }
                    }
                }
            }
            Long("help") | Short('h') => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    // Set command based on subcommand
    cli_args.command = match subcommand.as_deref() {
        Some("ui") => Command::Ui {
            file: ui_file.unwrap_or_else(|| PathBuf::from("simulation_events.json")),
        },
        Some("analyze") => Command::Analyze {
            file: analyze_file.unwrap_or_else(|| PathBuf::from("simulation_events.json")),
        },
        Some("compare") => {
            if compare_files.is_empty() {
                eprintln!("Error: compare command requires at least one file");
                std::process::exit(1);
            }
            Command::Compare {
                files: compare_files,
            }
        }
        Some("explain") => Command::Explain {
            file: explain_file.unwrap_or_else(|| PathBuf::from("simulation_events.json")),
        },
        Some("batch") => {
            if let Some(config) = batch_config {
                Command::Batch { config }
            } else {
                eprintln!("Error: batch command requires a configuration file");
                std::process::exit(1);
            }
        }
        Some("analyze-batch") => {
            if analyze_batch_files.is_empty() {
                eprintln!("Error: analyze-batch command requires at least one file");
                std::process::exit(1);
            }
            Command::AnalyzeBatch {
                files: analyze_batch_files,
                output: analyze_batch_output,
            }
        }
        Some("query") => {
            if let Some(file) = query_file {
                Command::Query {
                    file,
                    filters: query_filters,
                }
            } else {
                eprintln!("Error: query command requires a file");
                std::process::exit(1);
            }
        }
        Some("run") | None => Command::Run,
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            print_help();
            std::process::exit(1);
        }
    };

    Ok(cli_args)
}

/// Apply CLI overrides to a scenario's parameters.
pub fn apply_overrides(scenario: &mut Scenario, args: &CliArgs) {
    if let Some(days) = args.days {
        scenario.parameters.days_to_simulate = days;
    }

    if let Some(delay) = args.growth_delay {
        scenario.parameters.days_before_growth_chance = delay;
    }

    if let Some(seed) = args.random_seed {
        scenario.random_seed = Some(seed);
    }

    // Apply initial resource overrides to all villages
    for village in &mut scenario.villages {
        if let Some(food) = args.initial_food {
            village.initial_food = food;
        }
        if let Some(wood) = args.initial_wood {
            village.initial_wood = wood;
        }
        if let Some(money) = args.initial_money {
            village.initial_money = money;
        }
    }
}

/// Validate scenario configuration and print warnings.
pub fn validate_scenario(scenario: &Scenario, args: &CliArgs) {
    let params = &scenario.parameters;

    // Check growth timing issue
    if params.days_before_growth_chance >= params.days_to_simulate {
        println!(
            "\n⚠️  WARNING: Growth delay ({} days) >= simulation length ({} days)",
            params.days_before_growth_chance, params.days_to_simulate
        );
        println!("   Villages will not have time to grow population!");
        println!(
            "   Consider using --days {} or --growth-delay {}\n",
            params.days_before_growth_chance + 100,
            params.days_to_simulate / 2
        );
    }

    // Check for identical production slots with trading strategy
    if args.strategies.iter().any(|s| s == "trading") {
        let all_same_slots = scenario.villages.windows(2).all(|pair| {
            pair[0].food_slots == pair[1].food_slots && pair[0].wood_slots == pair[1].wood_slots
        });

        if all_same_slots && scenario.villages.len() > 1 {
            println!("⚠️  WARNING: All villages have identical production slots");
            println!("   Trading strategy may not specialize effectively!");
            println!("   Consider using different slot configurations\n");
        }
    }

    // Check for insufficient starting resources
    for village in scenario.villages.iter() {
        let min_food_needed = Decimal::from(village.initial_workers * 10);
        let min_wood_needed = Decimal::from(10); // For at least one house

        if village.initial_food < min_food_needed {
            println!(
                "⚠️  WARNING: Village {} has low initial food ({} < {} recommended)",
                village.id, village.initial_food, min_food_needed
            );
        }

        if village.initial_wood < min_wood_needed && village.initial_houses < 2 {
            println!(
                "⚠️  WARNING: Village {} has low initial wood ({} < {} recommended)",
                village.id, village.initial_wood, min_wood_needed
            );
        }
    }
}

fn print_help() {
    println!("\nVillage Model Simulation - Enhanced CLI\n");
    println!("USAGE:");
    println!("    village-model-sim [COMMAND] [OPTIONS]\n");

    println!("COMMANDS:");
    println!("    run              Run the simulation (default)");
    println!("    ui [FILE]        View simulation events in TUI");
    println!("    analyze [FILE]   Analyze simulation results");
    println!("    compare FILE...  Compare multiple simulation results");
    println!("    explain [FILE]   Generate narrative explanation of events");
    println!("    batch CONFIG     Run batch experiments from YAML config");
    println!("    analyze-batch FILE... [-o OUTPUT]  Analyze multiple results and export");
    println!("    query FILE [OPTIONS]  Query and filter simulation events\n");

    println!("SIMULATION OPTIONS:");
    println!("    -s, --strategy <NAME>      Strategy for villages (can be used multiple times)");
    println!("                               Available: default, survival, growth, trading,");
    println!("                               balanced, greedy");
    println!("    --scenario <NAME>          Use a built-in scenario (default: basic)");
    println!("    --scenario-file <FILE>     Load scenario from JSON file");
    println!("    -d, --days <N>             Number of days to simulate");
    println!("    --growth-delay <N>         Days before population growth possible");
    println!("    --seed <N>                 Random seed for reproducible runs");
    println!("    --initial-food <N>         Override initial food for all villages");
    println!("    --initial-wood <N>         Override initial wood for all villages");
    println!("    --initial-money <N>        Override initial money for all villages\n");

    println!("OUTPUT OPTIONS:");
    println!("    -o, --output <FILE>        Output events to specified file");
    println!("    --debug                    Enable debug output");
    println!("    -v, --verbose              Enable verbose output");
    println!("    -q, --quiet                Suppress non-essential output");
    println!("    --debug-decisions <ID>     Debug strategy decisions for specific village");
    println!("    -h, --help                 Print help information\n");

    println!("QUERY OPTIONS:");
    println!("    --village <ID>             Filter by village ID");
    println!("    --event-type <TYPE>        Filter by event type");
    println!("    --resource <TYPE>          Filter by resource type (food/wood)");
    println!("    --tick-range <START-END>   Filter by tick range (e.g., 0-100)\n");

    println!("UI CONTROLS:");
    println!("    Space            Pause/Resume playback");
    println!("    ←/→              Step backward/forward through events");
    println!("    Home/End         Jump to beginning/end");
    println!("    +/-              Faster/slower playback");
    println!("    Tab              Switch between views");
    println!("    Q                Quit\n");

    println!("EXAMPLES:");
    println!("    # Run with custom parameters");
    println!("    village-model-sim run -s trading -s balanced --days 200 --growth-delay 50\n");

    println!("    # Run with reproducible seed");
    println!("    village-model-sim run --seed 12345 --debug\n");

    println!("    # Analyze simulation results");
    println!("    village-model-sim analyze simulation_events.json\n");

    println!("    # Compare different strategies");
    println!("    village-model-sim compare survival.json growth.json trading.json");
}
