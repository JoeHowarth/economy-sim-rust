# Village Model Scenarios

This directory contains predefined scenarios for the village model simulation. Each scenario is a JSON file that defines initial conditions, parameters, and strategies for villages.

## Available Scenarios

### balanced_start.json
- **Description**: Two villages with balanced initial resources, ideal for testing different strategies
- **Villages**: 2 balanced villages with good starting resources
- **Duration**: 200 days
- **Key Features**: 
  - Equal production slots for both resources
  - Faster growth chance (after 50 days)
  - Good for comparing strategy performance

### resource_scarcity.json
- **Description**: Villages struggling with limited resources
- **Villages**: 2 villages (desert and mountain) with poor production
- **Duration**: 150 days
- **Key Features**:
  - Very limited production slots
  - Minimal starting resources
  - Tests survival under harsh conditions

### trading_specialization.json
- **Description**: Villages specialized in different resources to encourage trading
- **Villages**: 3 villages (food specialist, wood specialist, balanced trader)
- **Duration**: 250 days
- **Key Features**:
  - Extreme specialization (20 vs 5 production slots)
  - Tests trading mechanics and specialization benefits
  - Mixed strategies (Trading and Balanced)

### competitive_market.json
- **Description**: Four villages competing in a resource-constrained market
- **Villages**: 4 villages with different strategies (Greedy, Growth, Survival, Balanced)
- **Duration**: 300 days
- **Key Features**:
  - Multiple competing strategies
  - Moderate resource constraints
  - Tests strategy performance in competition

### abundance.json
- **Description**: Villages with abundant resources to test growth strategies
- **Villages**: 3 villages focused on expansion
- **Duration**: 400 days
- **Key Features**:
  - High starting resources and production slots
  - Faster growth (8% daily chance after 30 days)
  - Higher base production (1.2x)
  - Tests maximum growth potential

## Running Scenarios

To run a scenario from a JSON file:

```bash
cargo run -- run --scenario-file scenarios/balanced_start.json
```

You can override scenario parameters with CLI options:

```bash
# Run trading scenario for 500 days with a specific seed
cargo run -- run --scenario-file scenarios/trading_specialization.json --days 500 --seed 12345

# Override initial resources
cargo run -- run --scenario-file scenarios/resource_scarcity.json --initial-food 100 --initial-wood 100
```

## Scenario File Format

Each scenario file must include:

1. **name**: Scenario identifier
2. **description**: Human-readable description
3. **parameters**: Simulation parameters
   - `days_to_simulate`: How long to run
   - `days_without_food_before_starvation`: Starvation threshold
   - `days_without_shelter_before_death`: Exposure threshold
   - `days_before_growth_chance`: When population can start growing
   - `growth_chance_per_day`: Daily spawn probability (0.0-1.0)
   - `house_construction_days`: Time to build a house
   - `house_construction_wood`: Wood cost for houses
   - `house_capacity`: Workers per house
   - `house_decay_rate`: Daily maintenance decay
   - `base_food_production`: Base productivity for food
   - `base_wood_production`: Base productivity for wood
   - `second_slot_productivity`: Efficiency of second worker slot (0.0-1.0)
4. **random_seed** (optional): For reproducible runs
5. **villages**: Array of village configurations
   - `id`: Unique identifier
   - `initial_workers`: Starting population
   - `initial_houses`: Starting shelter
   - `initial_food`, `initial_wood`, `initial_money`: Starting resources
   - `food_slots`, `wood_slots`: Production capacity as [first_slot, second_slot]
   - `strategy`: Strategy configuration with type and parameters

## Strategy Types

- **Balanced**: Equal weights for all activities
- **Survival**: Focus on maintaining food/shelter buffers
- **Growth**: Maximize population expansion
- **Trading**: Specialize and trade aggressively
- **Greedy**: Maximize immediate production value

See the main documentation for detailed strategy descriptions.