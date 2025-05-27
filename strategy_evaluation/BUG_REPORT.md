# Bug Report: Village Model Strategy Issues

## Critical Bugs

### 1. Growth Mechanic Timing Issue
**Severity**: High  
**Impact**: No villages can achieve population growth in default scenarios

**Description**: 
The default simulation runs for 100 days, but workers need 100 days of continuous food+shelter before they can reproduce. This means growth can only START on day 100, but the simulation ends immediately.

**Evidence**:
- All simulations show 0-15% growth rates
- Log output shows "worker.days_with_both >= 100" but no "new worker" events until day 100+

**Fix**:
```rust
// In scenario.rs default parameters
days_to_simulate: 200,  // Was 100
days_before_growth_chance: 50,  // Was 100
```

### 2. Trading Strategy Specialization Failure
**Severity**: High  
**Impact**: Trading villages don't specialize properly when resources are equal

**Description**:
Trading strategy uses simple comparison `food_slots > wood_slots` but when they're equal (common in scenarios), specialization becomes random or biased.

**Evidence**:
- In trading_vs_trading with identical villages, both may specialize the same way
- No wood trades occur in some scenarios despite wood specialists

**Fix**:
```rust
// In strategies.rs TradingStrategy::new()
if food_slot_value == wood_slot_value {
    // Use village ID hash for deterministic specialization
    specialize_food = village_state.id.bytes().sum::<u8>() % 2 == 0;
} else {
    specialize_food = food_slot_value > wood_slot_value;
}
```

### 3. Missing Auction Clearing Events
**Severity**: Medium  
**Impact**: Cannot analyze price discovery or market dynamics

**Description**:
The auction system runs but doesn't log AuctionCleared events, making it impossible to track clearing prices over time.

**Evidence**:
- 0 AuctionCleared events in all simulations
- Trades occur but no price history available

**Fix**: Add logging in main.rs after auction execution

### 4. Inadequate Order Matching
**Severity**: Medium  
**Impact**: Very low trade volumes even with willing traders

**Description**:
Only 12 trades in 100 days for trading_vs_balanced scenario suggests orders aren't matching effectively.

**Evidence**:
- Village_a placed 100 sell orders, only 6 executed
- Village_b placed 106 orders (mixed), only 6 executed
- 94% of orders expire without matching

**Potential Causes**:
- Price gaps too large
- Quantity mismatches
- Budget constraints too restrictive

### 5. Single Resource Trading
**Severity**: Medium  
**Impact**: Limits market dynamics and strategy effectiveness

**Description**:
Analysis shows only Food trades occurred in mixed strategy scenarios, no Wood trades despite orders being placed.

**Evidence**:
```json
{
  "village_b": {
    "orders": {
      "food_orders": 6,
      "wood_orders": 100  // Placed but never executed
    }
  }
}
```

## Tooling Bugs

### 1. Event JSON Structure Inconsistency
**Severity**: Low  
**Impact**: Makes analysis scripts complex

**Description**:
Events use nested structure with type inside event_type object rather than flat structure, complicating JSON queries.

### 2. No Random Seed Logging
**Severity**: Low  
**Impact**: Cannot reproduce specific runs

**Description**:
Simulations don't log their random seed, making it impossible to reproduce interesting scenarios.

## Recommendations

1. **Immediate Fixes**:
   - Extend default simulation to 200 days
   - Reduce growth delay to 50 days
   - Add auction clearing event logging

2. **Testing Needed**:
   - Create unit tests for strategy specialization
   - Add integration tests for trading scenarios
   - Test edge cases (0 money, 0 resources, etc.)

3. **Monitoring**:
   - Add trade success rate metrics
   - Track order expiration reasons
   - Log price convergence patterns