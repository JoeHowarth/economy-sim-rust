# Simplified Village Simulation Specification (Revised)

This document outlines the rules for a simplified village simulation system. The simulation tracks how villages produce, consume, and trade resources (primarily **food** and **wood**) while trying to maximize population.

## 1. Overview

- **Resources**:  
  - *Base Resources*: **Wood** (harvestable/loggable) and **Food** (harvestable/growable)  
  - *Derived Resource*: **Homes** (constructed from wood)  
  - *Consumables*: **Food** (eaten daily) and **Homes** (needed for shelter)

- **Villages**:  
  - Each village has a certain number of workers and houses.  
  - Workers produce resources if allocated to tasks (food/wood gathering, home building, repairing).  
  - Each village can trade resources with all other villages (no connectivity limits).  
  - A village’s strategy determines how it allocates workers and decides on trading/buying/selling.

- **Population Goal**:  
  - The overarching objective is to grow or maintain as large a population as possible over a fixed simulation period (e.g. 2,000 days).

## 2. Village Structure

- **Workers**:  
  - Each worker is an individual who must be fed (1 unit of food/day) and housed (under a shared home).  
  - Workers die if unfed for 10 consecutive days or if left unsheltered for 30 consecutive days.  
  - Productivity for each worker is reduced by 20% for lack of food and by another 20% for lack of shelter (for a possible total −40%).  

- **Homes**:  
  - Each home can shelter up to 5 people if fully maintained.  
  - Building a new home requires **10 wood** and **60 worker-days** of construction effort.  
  - Houses require periodic maintenance: if not provided wood, `maintenance_level` decreases daily.  
    - If `maintenance_level >= 0`, the house provides full 5 capacity.  
    - If `maintenance_level` is negative, for each full integer of negative value, capacity is reduced by 1 (down to zero if the negative value is ≥ 5).  
  - Repairing a home involves supplying enough wood to bring `maintenance_level` back up to ≥ 0. This uses worker-days as well.

## 3. Resource Management

### 3.1 Daily Consumption
- Each worker consumes 1 unit of food per day if available.  
- If less than 1 unit of food is left, the worker goes hungry that day.

### 3.2 Production
- Villages gather wood or food by allocating workers to resource tasks.  
- **Diminishing Returns**:  
  - The *first* allocated worker slot yields 100% of base productivity.  
  - The *second* allocated slot yields 75% productivity.  
  - Additional workers beyond the second slot provide **no** further production.  

- **Base Productivity**:
  - **Wood**: 0.1 wood/day for a full slot  
  - **Food**: 2 food/day for a full slot  

> **Example**: If two full slots are allocated to wood, total wood = `0.1 * 1.0 + 0.1 * 0.75 = 0.175 wood/day`. If three or more workers are allocated, the third+ slots produce zero.

### 3.3 Trading
- All villages can directly trade resources with each other.  
- **No transaction losses** are applied (the original 10% loss has been removed).  
- Each village sets its own ask and bid prices for wood and food. Trades occur if a buyer’s bid price is above the seller’s ask price.

## 4. Worker Needs & Conditions

- **Food Requirement**:  
  - 1 food/day per worker.  
  - A worker starves after 10 consecutive days without food.

- **Shelter Requirement**:  
  - A worker must have a “bed” each night.  
  - A worker dies after 30 consecutive days without shelter.

- **Productivity Penalties**:  
  - If a worker had no food available on a given day, their productivity is reduced by 20% on that day.  
  - If they also had no shelter, that’s another −20%.

- **Population Growth**:  
  - If a worker has enjoyed both food and shelter for 100 consecutive days, there is a daily 5% chance of generating a new worker (representing population growth).

## 5. Simulation Flow

1. **Strategy Decisions**:  
   Each village’s strategy determines worker allocations for the day (wood gathering, food gathering, house building/repairs) and sets ask/bid prices for trades.

2. **Resource Production**:  
   Based on the day’s allocations and the diminishing-returns rule, villages produce wood/food.  
   - The first slot yields 100% productivity, the second yields 75%.  
   - No benefit from additional workers beyond slot #2.

3. **Trade**:  
   - All posted bids and asks across the villages are matched.  
   - Trades happen if the bid price > ask price.  
   - There is no transaction fee (0% loss).

4. **Consumption & Upkeep**:  
   - Workers each consume 1 unit of food if available.  
   - Houses consume wood for maintenance (daily or in chunks, per design). If insufficient, `maintenance_level` declines, potentially reducing capacity.

5. **Worker Updates**:  
   - If a worker could not get food, their `days_without_food` increases; otherwise reset to zero.  
   - If no shelter was available, `days_without_shelter` increases; otherwise reset to zero.  
   - If both food and shelter were available, `days_with_both` increments for that day.

6. **Population Growth & Mortality**:  
   - Any worker above 100 consecutive days with both food and shelter has a small chance to spawn a new worker.  
   - Workers who exceed 10 days of hunger or 30 days without shelter die and are removed.

7. **Repeat**:  
   - This cycle continues for a fixed number of days (e.g. 2,000).  
   - At the end, each village’s population count is recorded for comparison.

## 6. House Building & Maintenance

- **Building New Homes**:  
  - Construction requires **10 wood** and **60 worker-days** to complete.  
  - The strategy must decide how many worker-days to devote to building.  
  - Once completed, a new House object is added to the village.

- **Maintenance Tracking**:  
  - Each house has a floating `maintenance_level`.  
  - If `maintenance_level >= 0`, the house offers full capacity (5 persons).  
  - If `maintenance_level` is negative, capacity is `5 - floor(|maintenance_level|)`.  
    - Example: If `maintenance_level = -2.3`, the house capacity is `5 - 2 = 3`.  
    - Once capacity hits 0, the house is effectively broken.  
  - Daily or periodic upkeep uses wood to raise `maintenance_level` (and requires some worker-days for the repair action).

## 7. Strategy Requirements

Each village’s strategy must:

1. **Allocate Workers**: Decide how many workers gather wood, gather food, build new homes, or repair existing homes.  
2. **Set Trade Parameters**: Post daily asks and bids for wood and food.  
3. **Balance Survival & Growth**: Ensure enough food/housing for the existing population while planning for expansion (more houses) and possibly stockpiling.

## 8. Evaluation Criteria

- **Final Population** after the simulation (primary metric).  
- **Stability**: Avoiding massive die-offs.  
- **Robustness** to different starting conditions.  
- **Resource Utilization**: Effective allocations and home maintenance.

----

With these revisions, the simulation no longer applies a third worker slot at 50% efficiency, does not apply a 10% trading loss, and assumes every village can trade with every other. House building still requires 10 wood and 60 worker-days; the maintenance system uses a stepped penalty for negative `maintenance_level`.
