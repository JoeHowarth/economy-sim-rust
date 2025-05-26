# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based economic simulation of village resource management and trading. Villages produce food and wood, build homes, manage populations, and trade resources through a double auction system.

## Commands

**Build & Run:**
- `cargo build` - Build the project
- `cargo run --bin village-model-sim` - Run the simulation
- `cargo build --release` - Build optimized version

**Testing:**
- `cargo test` - Run all tests
- `cargo test [test_name]` - Run specific test
- `cargo test --lib` - Run library tests only

**Development:**
- `cargo check` - Fast type checking without building
- `cargo fmt` - Format code
- `cargo clippy` - Run linter

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