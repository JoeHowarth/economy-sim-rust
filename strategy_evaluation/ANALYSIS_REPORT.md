# Strategy Evaluation Analysis Report

## Executive Summary

After running multiple simulations with different strategy combinations, I've identified several key insights about strategy performance, bugs, and opportunities for tooling improvements.

## Key Findings

### 1. **No Trading Actually Occurs Between Same Strategies**
- When villages use the same strategy (e.g., survival vs survival), NO trades happen
- Total trade volume is 0 for all same-strategy matchups
- This suggests strategies may be too similar in their resource valuation

### 2. **Trading Strategy Dominates in Mixed Scenarios**
- Trading vs Balanced: Trading village achieved 2.09 score vs -0.52 for Balanced
- Trading strategy generates 12.48 profit per trade while Balanced loses the same amount
- Only 128 total trade volume occurred (very low for 100 days)

### 3. **Limited Trading Variety**
- Analysis of trading_vs_balanced shows only Food trades occurred (no Wood trades)
- Village_a (trading) only sold Food (100 ask orders, 6 executed)
- Village_b (balanced) placed mixed orders but only bought Food

### 4. **Population Growth Issues**
- Most strategies achieved minimal growth (0-15% over 100 days)
- Growth happens after day 100, but simulations end at day 100
- This creates a catch-22 where growth conditions are met but time runs out

### 5. **Strategy Performance Rankings** (by efficiency)
1. **Greedy**: 2.00 resources/worker/day (but 0% growth)
2. **Trading**: 1.63-1.71 resources/worker/day
3. **Survival**: 1.16-1.28 resources/worker/day
4. **Growth**: 1.18-1.24 resources/worker/day
5. **Balanced**: 1.10-1.20 resources/worker/day

## Identified Bugs and Issues

### 1. **Trading Strategy Specialization Bug**
The trading strategy decides specialization based on slot comparison:
```rust
let food_slot_value = village_state.food_slots.0 + village_state.food_slots.1;
let wood_slot_value = village_state.wood_slots.0 + village_state.wood_slots.1;
specialize_food: food_slot_value > wood_slot_value
```
But in the basic scenario, both villages have identical slots (10,10), so specialization is arbitrary.

### 2. **Auction System Integration Issue**
- No "AuctionCleared" events are logged
- The auction system appears to be running but not logging its clearing process
- This makes it impossible to analyze price discovery

### 3. **Order Type Inconsistency**
Orders are placed with "side" field but trades execute with different terminology, making analysis difficult.

### 4. **Growth Timing Problem**
The simulation parameters create an impossible situation:
- Growth requires 100 days of good conditions
- Simulation only runs 100 days
- Result: Almost no population growth occurs

## Tooling Improvement Recommendations

### 1. **Better Simulation Configuration**
```rust
// Add to CLI
--days <N>              // Custom simulation length
--growth-delay <N>      // Days before growth eligible
--start-resources <N>   // Override initial resources
--random-seed <N>       // For reproducible runs
```

### 2. **Enhanced Metrics and Debugging**
- Add per-tick price tracking
- Log why trades succeed/fail
- Track resource shortages by type
- Show strategy decision reasoning
- Add "strategy explanation" mode that logs why each decision was made

### 3. **Visualization Improvements**
The current TUI could benefit from:
- Price charts over time
- Trade flow visualization
- Resource balance graphs
- Strategy comparison view
- "Why did this happen?" explanatory mode

### 4. **Testing Framework**
```rust
// Proposed test scenarios
#[test]
fn test_strategy_achieves_growth() {
    let result = run_scenario(
        StrategyType::Growth,
        days: 200,  // Enough time for growth
        assert_min_growth: 0.5
    );
}

#[test] 
fn test_trading_creates_market() {
    let result = run_mixed_scenario(
        vec![Trading, Balanced],
        assert_min_trades: 50,
        assert_price_convergence: true
    );
}
```

### 5. **Strategy Analysis Tools**
```bash
# Proposed CLI commands
village-model analyze <events.json>    # Deep analysis
village-model compare <strat1> <strat2> # Head-to-head comparison
village-model optimize <strategy>       # Parameter tuning
village-model explain <events.json>     # Narrative explanation
```

### 6. **Configuration Validation**
Add warnings for problematic configurations:
- Growth delay >= simulation length
- Identical production slots with trading strategy
- Insufficient starting resources for strategy type

## Strategy-Specific Observations

### Survival Strategy
- Very conservative, maintains large buffers
- Rarely trades even when it would be beneficial
- Good for stability but poor for growth

### Growth Strategy  
- Allocates well for growth but can't achieve it due to timing
- Needs longer simulations to show its strengths
- Should be more aggressive about trading for needed resources

### Trading Strategy
- Dominates when it can specialize
- Needs better specialization logic for identical villages
- Should adapt when no trading partners exist

### Balanced Strategy
- Jack of all trades, master of none
- Loses in trading scenarios due to poor price negotiation
- Needs smarter market analysis

### Greedy Strategy
- Achieves highest efficiency but zero growth
- Never builds houses (as designed)
- Could benefit from emergency construction logic

## Recommendations for Next Steps

1. **Fix Growth Timing**: Either reduce growth delay or extend default simulation length
2. **Improve Trading**: Add more sophisticated price discovery and market making
3. **Add Logging**: Implement detailed decision logging for strategy debugging
4. **Create Scenarios**: Design specific test scenarios for each strategy type
5. **Build Analysis Tools**: Create automated tools to compare strategy performance
6. **Enhance Visualizations**: Add real-time charts and explanatory overlays

## Conclusion

The current implementation has solid foundations but needs refinement in strategy differentiation, market dynamics, and analysis tooling. The main issues are:
- Strategies are too similar in resource valuation
- Growth mechanics are poorly timed
- Trading lacks sophistication
- Analysis tools are primitive

With the suggested improvements, the simulation would become a much more powerful tool for understanding emergent economic behaviors.