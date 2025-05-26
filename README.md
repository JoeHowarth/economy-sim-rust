# economy-sim-rust

A village economics simulator where small populations struggle against thermodynamics and each other.

## What is this?

I got curious about the minimal conditions for sustainable civilization. Not the grand sweep of history stuff - just the basics. How many people do you need to keep everyone fed and sheltered? What happens when you add trade? When does specialization emerge from necessity?

So I built a simulator. Villages manage wood and food production with diminishing returns (because the best trees get cut first, and the best land gets farmed first). Workers need food daily or they starve in 10 days. They need shelter or they die of exposure in 30. Houses decay without maintenance. Population grows at 5% when conditions are good.

The interesting part: villages can trade through a double auction system with budget constraints and order pruning. No central planning, just local decisions and price discovery.

## The Model

Each village allocates workers between:
- Food production (2 units/day for first worker, 1.5 for second)  
- Wood production (0.1/day first, 0.075 second)
- House construction (60 worker-days + 10 wood per house)
- House maintenance (0.1 wood/day or decay happens)

The auction system uses a proper iterative clearing algorithm that handles multiple resources and prevents budget violations. Think of it as a tiny commodities exchange where failure means death.

## Running it

```bash
cargo run
```

## Architecture Notes

The separation between simulation logic (`main.rs`) and market mechanics (`auction.rs`) is clean. Villages make local decisions through a strategy trait. 
The auction runs as a separate system that could theoretically handle any tradeable resources.

## Status

Active development. The basics work but the interesting emergent behaviors aren't there yet. Need to:
- Implement actual trading (not just price discovery)
- Add more sophisticated village strategies 
- Run longer simulations to see if stable states exist
- Maybe add geography/trade routes for more realistic constraints

PRs welcome if you're into this sort of thing.
