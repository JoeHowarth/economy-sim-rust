# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based economic simulation of village resource management and trading. Villages produce food and wood, build homes, manage populations, and trade resources through a double auction system.

## Commands

**Build & Run:**
- `cargo build` - Build the project
- `cargo run --bin village-model-sim` - Run the simulation
- `cargo run -- run -s survival -s growth` - Run with specific strategies
- `cargo run -- run --scenario trading` - Run specific scenario
- `cargo build --release` - Build optimized version

**Testing:**
- `cargo test` - Run all tests
- `cargo test [test_name]` - Run specific test
- `cargo test --lib` - Run library tests only

**Development:**
- `cargo check` - Fast type checking without building
- `cargo fmt` - Format code
- `cargo clippy` - Run linter
- Run clippy before committing 

## Available Strategies

**Default:** Fixed 70% wood, 20% food, 10% construction allocation. No trading.

**Survival:** Prioritizes immediate needs (food/shelter). Maintains 20-day food buffer and 10-day wood buffer. Only builds when resources are stable.

**Growth:** Focuses on population expansion. Balances food/housing for growth. Target 3.5:1 worker-to-house ratio. Trades for needed resources.

**Trading:** Specializes in one resource based on production slots. Aggressive trading (30% of surplus). Minimal construction effort.

**Balanced:** Adapts allocation based on current needs. Dynamic weights based on resource urgency. Moderate trading with 15-day buffers.

**Greedy:** Maximizes immediate production value. No construction. Emergency trades only at premium prices.

## Architecture

**Core Simulation Loop (src/main.rs):**
- Villages allocate workers to tasks (food/wood gathering, home building/repair)
- Resources are produced with diminishing returns (2 worker slots max)
- Trading occurs through auction system
- Population dynamics based on food/shelter availability

**Auction System (src/auction.rs):**
- Double auction with bid/ask matching
- Budget constraint enforcement with order pruning
- Multi-resource clearing (wood and food markets)
- Uses rust_decimal for precise financial calculations
- Both wood and food trading are fully functional

**Strategy System (src/strategies.rs):**
- Modular strategy pattern with different village behaviors
- Each strategy implements worker allocation and trading decisions
- Strategies can be assigned via CLI or scenario configuration
- All strategies can generate both wood and food orders

**Key Design Patterns:**
- Strategy pattern for village AI decisions
- Component-based entities (Worker, House, Village)
- Separation of auction logic from simulation logic

## Important Implementation Details

**Resource Production:**
- First worker slot: 100% productivity
- Second worker slot: 75% productivity  
- Additional workers: 0% productivity

**Population Mechanics:**
- Workers need 1 food/day or starve after 10 days
- Workers need shelter or die after 30 days
- New workers spawn with 5% daily chance after 100 days of food+shelter

**House System:**
- Building: 10 wood + 60 worker-days
- Capacity: 5 workers when maintained
- Maintenance level decreases daily, reducing capacity when negative

## Git Guidelines
- Use git effectively. Do not make 'backup' files