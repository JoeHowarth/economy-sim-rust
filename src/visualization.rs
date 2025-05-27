//! ASCII-based visualization tools for simulation data.

use crate::analysis::{PriceHistory, SimulationAnalysis};
use crate::events::TradeSide;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Generate an ASCII price chart.
pub fn price_chart(price_history: &PriceHistory, width: usize, height: usize) -> String {
    let mut chart = String::new();

    chart.push_str("Price History\n");
    chart.push_str(&"─".repeat(width));
    chart.push('\n');

    // Combine and sort all prices
    let mut all_prices: Vec<(usize, Decimal, &str)> = Vec::new();
    for (tick, price) in &price_history.wood_prices {
        all_prices.push((*tick, *price, "W"));
    }
    for (tick, price) in &price_history.food_prices {
        all_prices.push((*tick, *price, "F"));
    }
    all_prices.sort_by_key(|(tick, _, _)| *tick);

    if all_prices.is_empty() {
        chart.push_str("No price data available\n");
        return chart;
    }

    // Find min/max for scaling
    let max_price = all_prices
        .iter()
        .map(|(_, p, _)| *p)
        .max()
        .unwrap_or(Decimal::ZERO);
    let min_price = all_prices
        .iter()
        .map(|(_, p, _)| *p)
        .min()
        .unwrap_or(Decimal::ZERO);
    let max_tick = all_prices.iter().map(|(t, _, _)| *t).max().unwrap_or(0);

    // Create the chart
    let price_range = max_price - min_price;
    let price_scale = if price_range > Decimal::ZERO {
        Decimal::from(height - 3) / price_range
    } else {
        Decimal::ONE
    };

    // Initialize grid
    let mut grid: Vec<Vec<char>> = vec![vec![' '; width]; height];

    // Draw axes
    for row in grid.iter_mut().take(height) {
        row[0] = '│';
    }
    for x in 0..width {
        grid[height - 1][x] = '─';
    }
    grid[height - 1][0] = '└';

    // Plot prices
    for (tick, price, resource) in &all_prices {
        let x = (*tick * (width - 2) / max_tick.max(1)) + 1;
        let y = height - 2 - ((price - min_price) * price_scale).to_usize().unwrap_or(0);

        if x < width && y < height - 1 {
            grid[y][x] = match *resource {
                "W" => '●',
                "F" => '○',
                _ => '·',
            };
        }
    }

    // Add labels
    chart.push_str(&format!("{:>6.2} ┤", max_price));
    for x in 1..width {
        chart.push(grid[0][x]);
    }
    chart.push('\n');

    for (y, row) in grid.iter().enumerate().take(height - 1).skip(1) {
        if y == height / 2 {
            let mid_price = min_price + (max_price - min_price) / Decimal::from(2);
            chart.push_str(&format!("{:>6.2} ┤", mid_price));
        } else {
            chart.push_str("       │");
        }
        for &ch in row.iter().take(width).skip(1) {
            chart.push(ch);
        }
        chart.push('\n');
    }

    chart.push_str(&format!("{:>6.2} └", min_price));
    for _x in 1..width {
        chart.push('─');
    }
    chart.push_str("\n       0");
    let tick_label = format!("{}", max_tick);
    let padding = width - 8 - tick_label.len();
    chart.push_str(&" ".repeat(padding));
    chart.push_str(&tick_label);
    chart.push_str("\n\n● Wood  ○ Food\n");

    chart
}

/// Generate a population bar chart.
pub fn population_chart(analysis: &SimulationAnalysis, width: usize) -> String {
    let mut chart = String::new();

    chart.push_str("Population Changes\n");
    chart.push_str(&"─".repeat(width));
    chart.push('\n');

    // Find max population for scaling
    let max_pop = analysis
        .villages
        .iter()
        .map(|v| v.peak_population)
        .max()
        .unwrap_or(1);

    for village in &analysis.villages {
        let name = if village.id.len() > 15 {
            &village.id[..15]
        } else {
            &village.id
        };

        chart.push_str(&format!("{:>15} │", name));

        // Initial population bar
        let initial_width = (village.initial_population * (width - 20) / max_pop).min(width - 20);
        for _ in 0..initial_width {
            chart.push('░');
        }

        // Growth/decline bar
        let change = village.final_population as i32 - village.initial_population as i32;
        if change > 0 {
            let growth_width =
                (change as usize * (width - 20) / max_pop).min(width - 20 - initial_width);
            for _ in 0..growth_width {
                chart.push('█');
            }
        }

        chart.push_str(&format!(
            " {} → {} ({:+.0}%)\n",
            village.initial_population,
            village.final_population,
            village.growth_rate * 100.0
        ));
    }

    chart.push_str("\n░ Initial  █ Growth\n");
    chart
}

/// Generate a trade flow diagram.
pub fn trade_flow_diagram(analysis: &SimulationAnalysis) -> String {
    let mut diagram = String::new();

    diagram.push_str("Trade Flow Summary\n");
    diagram.push_str(&"═".repeat(50));
    diagram.push('\n');

    let mut any_trades = false;

    for village in &analysis.villages {
        if village.trading_summary.total_trades == 0 {
            continue;
        }

        any_trades = true;
        diagram.push_str(&format!("\n{}\n", village.id));
        diagram.push_str(&"─".repeat(village.id.len()));
        diagram.push('\n');

        if village.trading_summary.executed_sells > 0 {
            diagram.push_str(&format!(
                "  → SOLD: {} orders ({:.2} earned)\n",
                village.trading_summary.executed_sells, village.trading_summary.total_earned
            ));
        }

        if village.trading_summary.executed_buys > 0 {
            diagram.push_str(&format!(
                "  ← BOUGHT: {} orders ({:.2} spent)\n",
                village.trading_summary.executed_buys, village.trading_summary.total_spent
            ));
        }

        let net = village.trading_summary.net_profit;
        if net != Decimal::ZERO {
            diagram.push_str(&format!(
                "  {} NET: {:.2}\n",
                if net > Decimal::ZERO { "↑" } else { "↓" },
                net.abs()
            ));
        }
    }

    if !any_trades {
        diagram.push_str("\nNo trades executed during simulation.\n");
    } else {
        diagram.push_str(&format!(
            "\nTotal Market Volume: {} trades\n",
            analysis.market.total_trades
        ));
        diagram.push_str(&format!(
            "Trade Success Rate: {:.1}%\n",
            analysis.market.trade_success_rate * 100.0
        ));
    }

    diagram
}

/// Generate a resource balance timeline.
pub fn resource_timeline(
    events: &[crate::events::Event],
    village_id: &str,
    width: usize,
) -> String {
    use crate::events::EventType;

    let mut timeline = String::new();
    timeline.push_str(&format!("Resource Timeline: {}\n", village_id));
    timeline.push_str(&"─".repeat(width));
    timeline.push('\n');

    let mut food_balance = Decimal::ZERO;
    let mut wood_balance = Decimal::ZERO;
    let mut food_history = Vec::new();
    let mut wood_history = Vec::new();

    // Build resource history
    for event in events {
        if event.village_id != village_id {
            continue;
        }

        match &event.event_type {
            EventType::ResourceProduced {
                resource, amount, ..
            } => match resource {
                crate::events::ResourceType::Food => food_balance += amount,
                crate::events::ResourceType::Wood => wood_balance += amount,
            },
            EventType::ResourceConsumed {
                resource, amount, ..
            } => match resource {
                crate::events::ResourceType::Food => food_balance -= amount,
                crate::events::ResourceType::Wood => wood_balance -= amount,
            },
            EventType::TradeExecuted {
                resource,
                quantity,
                side,
                ..
            } => {
                let qty = *quantity;
                match (resource, side) {
                    (crate::events::ResourceType::Food, TradeSide::Buy) => food_balance += qty,
                    (crate::events::ResourceType::Food, TradeSide::Sell) => food_balance -= qty,
                    (crate::events::ResourceType::Wood, TradeSide::Buy) => wood_balance += qty,
                    (crate::events::ResourceType::Wood, TradeSide::Sell) => wood_balance -= qty,
                }
            }
            _ => {}
        }

        // Sample periodically
        if event.tick % 5 == 0 {
            food_history.push((event.tick, food_balance));
            wood_history.push((event.tick, wood_balance));
        }
    }

    if food_history.is_empty() {
        timeline.push_str("No resource data available for this village.\n");
        return timeline;
    }

    // Find scales
    let max_food = food_history
        .iter()
        .map(|(_, b)| *b)
        .max()
        .unwrap_or(Decimal::ONE);
    let max_wood = wood_history
        .iter()
        .map(|(_, b)| *b)
        .max()
        .unwrap_or(Decimal::ONE);
    let max_resource = max_food.max(max_wood);
    let max_tick = food_history.last().map(|(t, _)| *t).unwrap_or(100);

    // Draw mini chart
    let chart_height = 10;
    let chart_width = width - 10;

    for h in (0..chart_height).rev() {
        let threshold = max_resource * Decimal::from(h) / Decimal::from(chart_height);
        timeline.push_str(&format!("{:>6.0} │", threshold));

        for x in 0..chart_width {
            let tick = x * max_tick / chart_width;
            let food_at_tick = interpolate_value(&food_history, tick);
            let wood_at_tick = interpolate_value(&wood_history, tick);

            if food_at_tick >= threshold && wood_at_tick >= threshold {
                timeline.push('█');
            } else if food_at_tick >= threshold {
                timeline.push('F');
            } else if wood_at_tick >= threshold {
                timeline.push('W');
            } else {
                timeline.push(' ');
            }
        }
        timeline.push('\n');
    }

    timeline.push_str("       └");
    timeline.push_str(&"─".repeat(chart_width));
    timeline.push('\n');
    timeline.push_str(&format!(
        "        0{:>width$}{}\n",
        max_tick,
        "",
        width = chart_width - 2
    ));
    timeline.push_str("\nF=Food  W=Wood  █=Both\n");

    timeline
}

/// Generate a strategy performance matrix.
pub fn strategy_matrix(analyses: &[SimulationAnalysis]) -> String {
    let mut matrix = String::new();

    matrix.push_str("Strategy Performance Matrix\n");
    matrix.push_str(&"═".repeat(60));
    matrix.push('\n');

    // Collect all unique strategies
    let mut strategies = std::collections::HashSet::new();
    for analysis in analyses {
        for village in &analysis.villages {
            strategies.insert(village.id.clone()); // Using village ID as proxy for strategy
        }
    }

    // Header
    matrix.push_str(&format!(
        "{:>15} │ {:>8} │ {:>8} │ {:>8} │ {:>8}\n",
        "Strategy", "Growth%", "Survival%", "Trades", "Profit"
    ));
    matrix.push_str(&format!(
        "{:─>15}─┼{:─>10}┼{:─>10}┼{:─>10}┼{:─>10}\n",
        "", "", "", "", ""
    ));

    // Data rows
    for strategy in strategies {
        let mut growth_rates = Vec::new();
        let mut survival_rates = Vec::new();
        let mut trade_counts = Vec::new();
        let mut profits = Vec::new();

        for analysis in analyses {
            for village in &analysis.villages {
                if village.id == strategy {
                    growth_rates.push(village.growth_rate);
                    survival_rates.push(village.survival_rate);
                    trade_counts.push(village.trading_summary.total_trades);
                    profits.push(village.trading_summary.net_profit);
                }
            }
        }

        if !growth_rates.is_empty() {
            let avg_growth = growth_rates.iter().sum::<f64>() / growth_rates.len() as f64;
            let avg_survival = survival_rates.iter().sum::<f64>() / survival_rates.len() as f64;
            let avg_trades = trade_counts.iter().sum::<usize>() / trade_counts.len();
            let avg_profit = profits.iter().sum::<Decimal>() / Decimal::from(profits.len());

            matrix.push_str(&format!(
                "{:>15} │ {:>8.1} │ {:>8.1} │ {:>8} │ {:>8.2}\n",
                strategy,
                avg_growth * 100.0,
                avg_survival * 100.0,
                avg_trades,
                avg_profit
            ));
        }
    }

    matrix
}

// Helper function to interpolate values
fn interpolate_value(history: &[(usize, Decimal)], tick: usize) -> Decimal {
    if history.is_empty() {
        return Decimal::ZERO;
    }

    // Find surrounding points
    let mut before = None;
    let mut after = None;

    for (t, val) in history {
        if *t <= tick {
            before = Some((*t, *val));
        }
        if *t >= tick && after.is_none() {
            after = Some((*t, *val));
        }
    }

    match (before, after) {
        (Some((_, val)), None) => val,
        (None, Some((_, val))) => val,
        (Some((t1, v1)), Some((t2, v2))) => {
            if t1 == t2 {
                v1
            } else {
                // Linear interpolation
                let ratio = Decimal::from(tick - t1) / Decimal::from(t2 - t1);
                v1 + (v2 - v1) * ratio
            }
        }
        (None, None) => Decimal::ZERO,
    }
}
