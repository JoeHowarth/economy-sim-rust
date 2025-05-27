//! Query and filter simulation events.

use crate::cli::QueryFilters;
use crate::events::{Event, EventType, ResourceType, TradeSide};
use rust_decimal::Decimal;
use serde_json;
use std::fs;
use std::path::Path;

/// Query events from a simulation file with filters
pub fn query_events(file: &Path, filters: &QueryFilters) -> Result<Vec<Event>, String> {
    // Load events
    let contents = fs::read_to_string(file).map_err(|e| format!("Failed to read file: {}", e))?;

    let events: Vec<Event> =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Apply filters
    let filtered = events
        .into_iter()
        .filter(|event| {
            // Village filter
            if let Some(ref village) = filters.village {
                if event.village_id != *village {
                    return false;
                }
            }

            // Event type filter
            if let Some(ref event_type) = filters.event_type {
                if !event_matches_type(&event.event_type, event_type) {
                    return false;
                }
            }

            // Resource filter
            if let Some(ref resource) = filters.resource {
                if !event_has_resource(&event.event_type, resource) {
                    return false;
                }
            }

            // Tick range filter
            if let Some((start, end)) = filters.tick_range {
                if event.tick < start || event.tick > end {
                    return false;
                }
            }

            true
        })
        .collect();

    Ok(filtered)
}

/// Check if an event matches the given type string
fn event_matches_type(event_type: &EventType, type_str: &str) -> bool {
    let type_lower = type_str.to_lowercase();

    match event_type {
        EventType::WorkerAllocation { .. } => {
            type_lower.contains("allocation") || type_lower.contains("worker")
        }
        EventType::ResourceProduced { .. } => {
            type_lower.contains("produced") || type_lower.contains("production")
        }
        EventType::ResourceConsumed { .. } => {
            type_lower.contains("consumed") || type_lower.contains("consumption")
        }
        EventType::TradeExecuted { .. } => {
            type_lower.contains("trade") || type_lower.contains("executed")
        }
        EventType::OrderPlaced { .. } => {
            type_lower.contains("order") || type_lower.contains("placed")
        }
        EventType::WorkerBorn { .. } => type_lower.contains("born") || type_lower.contains("birth"),
        EventType::WorkerDied { .. } => type_lower.contains("died") || type_lower.contains("death"),
        EventType::HouseCompleted { .. } => {
            type_lower.contains("house") || type_lower.contains("completed")
        }
        EventType::VillageStateSnapshot { .. } => {
            type_lower.contains("snapshot") || type_lower.contains("state")
        }
        EventType::HouseDecayed { .. } => {
            type_lower.contains("decay") || type_lower.contains("house")
        }
        EventType::AuctionCleared { .. } => {
            type_lower.contains("auction") || type_lower.contains("clear") || type_lower.contains("market")
        }
    }
}

/// Check if an event involves the given resource
fn event_has_resource(event_type: &EventType, resource_str: &str) -> bool {
    let resource_lower = resource_str.to_lowercase();
    let is_food = resource_lower.contains("food");
    let is_wood = resource_lower.contains("wood");

    match event_type {
        EventType::ResourceProduced { resource, .. }
        | EventType::ResourceConsumed { resource, .. } => match resource {
            ResourceType::Food => is_food,
            ResourceType::Wood => is_wood,
        },
        EventType::TradeExecuted { resource, .. } => match resource {
            ResourceType::Food => is_food,
            ResourceType::Wood => is_wood,
        },
        EventType::OrderPlaced { resource, .. } => match resource {
            ResourceType::Food => is_food,
            ResourceType::Wood => is_wood,
        },
        _ => false,
    }
}

/// Format query results for display
pub fn format_query_results(events: &[Event], verbose: bool) -> String {
    let mut output = String::new();

    output.push_str(&format!("Found {} events\n", events.len()));
    output.push_str(&"─".repeat(50));
    output.push('\n');

    if events.is_empty() {
        output.push_str("No events match the specified filters.\n");
        return output;
    }

    // Group by event type for summary
    let mut type_counts = std::collections::HashMap::new();
    for event in events {
        let type_name = match &event.event_type {
            EventType::WorkerAllocation { .. } => "WorkerAllocation",
            EventType::ResourceProduced { .. } => "ResourceProduced",
            EventType::ResourceConsumed { .. } => "ResourceConsumed",
            EventType::TradeExecuted { .. } => "TradeExecuted",
            EventType::OrderPlaced { .. } => "OrderPlaced",
            EventType::WorkerBorn { .. } => "WorkerBorn",
            EventType::WorkerDied { .. } => "WorkerDied",
            EventType::HouseCompleted { .. } => "HouseCompleted",
            EventType::VillageStateSnapshot { .. } => "VillageStateSnapshot",
            EventType::HouseDecayed { .. } => "HouseDecayed",
            EventType::AuctionCleared { .. } => "AuctionCleared",
        };
        *type_counts.entry(type_name).or_insert(0) += 1;
    }

    output.push_str("\nEvent Type Summary:\n");
    for (event_type, count) in type_counts.iter() {
        output.push_str(&format!("  {}: {}\n", event_type, count));
    }

    if verbose {
        output.push_str("\nDetailed Events:\n");
        for (i, event) in events.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&format!("[Tick {}] {}: ", event.tick, event.village_id));
            output.push_str(&format_event_details(&event.event_type));
            output.push('\n');
        }
    } else {
        output.push_str("\nUse --verbose to see detailed event information.\n");
    }

    output
}

/// Format event details for display
fn format_event_details(event_type: &EventType) -> String {
    match event_type {
        EventType::WorkerAllocation {
            food_workers,
            wood_workers,
            construction_workers,
            ..
        } => {
            format!(
                "Allocated workers: {} food, {} wood, {} construction",
                food_workers, wood_workers, construction_workers
            )
        }
        EventType::ResourceProduced {
            resource,
            amount,
            workers_assigned,
            ..
        } => {
            format!(
                "Produced {:.2} {:?} with {} workers",
                amount, resource, workers_assigned
            )
        }
        EventType::ResourceConsumed {
            resource,
            amount,
            purpose,
            ..
        } => {
            format!("Consumed {:.2} {:?} for {:?}", amount, resource, purpose)
        }
        EventType::TradeExecuted {
            resource,
            quantity,
            price,
            side,
            ..
        } => {
            format!(
                "{:?} {} {:?} at {:.2} each",
                side, quantity, resource, price
            )
        }
        EventType::OrderPlaced {
            resource,
            quantity,
            price,
            side,
            ..
        } => {
            format!(
                "Placed {:?} order for {} {:?} at {:.2}",
                side, quantity, resource, price
            )
        }
        EventType::WorkerBorn { worker_id, .. } => {
            format!("Worker {} was born", worker_id)
        }
        EventType::WorkerDied {
            worker_id, cause, ..
        } => {
            format!("Worker {} died from {:?}", worker_id, cause)
        }
        EventType::HouseCompleted { house_id, .. } => {
            format!("House {} completed", house_id)
        }
        EventType::VillageStateSnapshot {
            population,
            food,
            wood,
            money,
            houses,
            ..
        } => {
            format!(
                "Snapshot: {} pop, {:.1} food, {:.1} wood, {:.1} money, {} houses",
                population, food, wood, money, houses
            )
        }
        EventType::HouseDecayed { house_id, .. } => {
            format!("House {} decayed", house_id)
        }
        EventType::AuctionCleared { wood_price, food_price, wood_volume, food_volume, .. } => {
            format!("Auction cleared - Wood: {} @ {:?}, Food: {} @ {:?}", 
                wood_volume, wood_price, food_volume, food_price)
        }
    }
}

/// Export query results to CSV
pub fn export_to_csv(events: &[Event], output: &Path) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        fs::File::create(output).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write header
    writeln!(file, "tick,village_id,event_type,details")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    // Write events
    for event in events {
        let type_name = match &event.event_type {
            EventType::WorkerAllocation { .. } => "WorkerAllocation",
            EventType::ResourceProduced { .. } => "ResourceProduced",
            EventType::ResourceConsumed { .. } => "ResourceConsumed",
            EventType::TradeExecuted { .. } => "TradeExecuted",
            EventType::OrderPlaced { .. } => "OrderPlaced",
            EventType::WorkerBorn { .. } => "WorkerBorn",
            EventType::WorkerDied { .. } => "WorkerDied",
            EventType::HouseCompleted { .. } => "HouseCompleted",
            EventType::VillageStateSnapshot { .. } => "VillageStateSnapshot",
            EventType::HouseDecayed { .. } => "HouseDecayed",
            EventType::AuctionCleared { .. } => "AuctionCleared",
        };

        let details = format_event_details(&event.event_type);
        writeln!(
            file,
            "{},{},\"{}\",\"{}\"",
            event.tick, event.village_id, type_name, details
        )
        .map_err(|e| format!("Failed to write CSV row: {}", e))?;
    }

    Ok(())
}

/// Generate a resource balance timeline.
pub fn resource_timeline(
    events: &[crate::events::Event],
    village_id: &str,
    width: usize,
) -> String {
    use crate::events::EventType;
    use rust_decimal::Decimal;

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
