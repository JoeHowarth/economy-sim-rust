# Test Audit Report

## Active Tests (Currently Passing)

### 1. `test_village_update_basic_production` ✅ KEEP
**Purpose**: Tests basic resource production mechanics
**Value**: Essential - validates core game mechanics
**Recommendation**: Keep as-is. This is a fundamental test.

### 2. `test_village_update_partial_slots` ✅ KEEP
**Purpose**: Tests diminishing returns (50% efficiency for partial slots)
**Value**: Important - validates a key game balance mechanism
**Recommendation**: Keep as-is. Critical for game balance.

### 3. `test_village_update_worker_states` ✅ KEEP
**Purpose**: Tests worker state tracking (food/shelter/both)
**Value**: Essential - validates worker condition tracking
**Recommendation**: Keep as-is. Core mechanic.

### 4. `test_village_update_no_resources` ✅ KEEP
**Purpose**: Tests behavior when village has no resources
**Value**: Important edge case
**Recommendation**: Keep but could be enhanced to test multiple days of starvation.

### 5. `test_village_update_house_maintenance` ✅ KEEP & REFACTOR
**Purpose**: Tests house maintenance mechanics in 3 scenarios
**Value**: Essential - covers maintenance system
**Recommendation**: Keep but split into 3 separate tests for clarity:
- `test_house_maintenance_no_wood`
- `test_house_maintenance_with_production`
- `test_house_maintenance_repair`

## Commented Tests (Need Review)

### 6. `test_village_update_worker_productivity` ❌ REMOVE
**Purpose**: Tests reduced productivity when worker lacks food/shelter
**Value**: Low - this is already tested implicitly in production tests
**Issue**: The test has incorrect productivity calculation (0.6 instead of actual)
**Recommendation**: REMOVE - productivity is implementation detail, not public API

### 7. `test_village_update_worker_no_shelter` ✅ KEEP & FIX
**Purpose**: Tests worker death after 30 days without shelter
**Value**: Essential - validates critical game rule
**Recommendation**: KEEP and fix. This is a core game mechanic from spec.

### 8. `test_village_update_worker_starvation` ✅ KEEP & FIX
**Purpose**: Tests worker death after 10 days without food
**Value**: Essential - validates critical game rule
**Recommendation**: KEEP and fix. Core mechanic from spec.

### 9. `test_village_update_worker_growth` ⚠️ FLAKY - REDESIGN
**Purpose**: Tests population growth after 100+ days with food+shelter
**Value**: Important but test is flaky (relies on RNG)
**Issue**: Uses random chance (5%) - will fail intermittently
**Recommendation**: REDESIGN to use a seeded RNG or mock the random function

### 10. `test_village_update_growth_chance` ❌ REMOVE
**Purpose**: Tests the growth_chance() method returns 0.05
**Value**: Low - tests implementation detail
**Issue**: The growth_chance() method is never used and marked as dead code
**Recommendation**: REMOVE - testing unused internal method

### 11. `test_house_construction_basic` ✅ KEEP & FIX
**Purpose**: Tests basic house construction (60 worker-days + 10 wood)
**Value**: Essential - validates house building mechanics
**Recommendation**: KEEP and fix. Core game mechanic.

### 12. `test_house_construction_partial` ❌ REMOVE OR MERGE
**Purpose**: Tests partial progress toward house construction
**Value**: Medium - already covered by basic test
**Recommendation**: MERGE into `test_house_construction_basic` as it's testing the same mechanism

### 13. `test_house_construction_insufficient_wood` ✅ KEEP & FIX
**Purpose**: Tests that construction progress accumulates but house isn't built without wood
**Value**: Important edge case
**Recommendation**: KEEP and fix. Important boundary condition.

### 14. `test_house_construction_multiple` ⚠️ QUESTIONABLE
**Purpose**: Tests building multiple houses in one update
**Value**: Low - unrealistic scenario
**Issue**: Having 130 workers with 0 houses is unrealistic game state
**Recommendation**: REMOVE - focus on realistic scenarios

### 15. `test_house_construction_with_other_allocations` ❌ REMOVE
**Purpose**: Tests construction progress while also allocating to other tasks
**Value**: Low - already covered by basic allocation tests
**Recommendation**: REMOVE - redundant with existing allocation tests

## Summary Recommendations

### Tests to Keep (9 total):
1. ✅ test_village_update_basic_production
2. ✅ test_village_update_partial_slots  
3. ✅ test_village_update_worker_states
4. ✅ test_village_update_no_resources
5. ✅ test_village_update_house_maintenance (split into 3)
6. ✅ test_village_update_worker_no_shelter (fix)
7. ✅ test_village_update_worker_starvation (fix)
8. ✅ test_house_construction_basic (fix)
9. ✅ test_house_construction_insufficient_wood (fix)

### Tests to Remove (5 total):
1. ❌ test_village_update_worker_productivity
2. ❌ test_village_update_growth_chance
3. ❌ test_house_construction_partial
4. ❌ test_house_construction_multiple
5. ❌ test_house_construction_with_other_allocations

### Tests to Redesign (1 total):
1. ⚠️ test_village_update_worker_growth (make deterministic)

## Missing Test Coverage

After this audit, we're still missing tests for:
1. **Trading functionality** - apply_trades is stubbed
2. **Strategy pattern** - only DefaultStrategy exists
3. **Multi-day simulations** - important for emergent behavior
4. **House capacity effects** - when maintenance_level < 0
5. **Edge cases** - negative resources, very large numbers
6. **Integration tests** - full game scenarios

## Proposed New Test Structure

```
Basic Mechanics (unit tests):
- Production (wood, food)
- Worker states (food, shelter, both)
- House maintenance
- Population dynamics (death, growth)
- Construction

Game Rules (integration tests):
- 10-day starvation
- 30-day exposure
- Population growth mechanics
- Resource constraints

Scenarios (end-to-end tests):
- Sustainable village
- Collapse scenarios
- Growth scenarios
- Trading scenarios
```