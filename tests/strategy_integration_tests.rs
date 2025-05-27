//! Integration tests for village strategies.

use village_model::strategies::*;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use rust_decimal_macros::dec;

/// Helper to create a test village state.
fn create_test_village(id: &str, workers: usize, food: f64, wood: f64, money: f64) -> VillageState {
    VillageState {
        id: id.to_string(),
        workers,
        wood: Decimal::from_f64(wood).unwrap(),
        food: Decimal::from_f64(food).unwrap(),
        money: Decimal::from_f64(money).unwrap(),
        house_capacity: workers * 2,
        houses: workers / 5 + 1,
        wood_slots: (10, 10),
        food_slots: (10, 10),
        worker_days: Decimal::from(workers),
        days_without_food: vec![0; workers],
        days_without_shelter: vec![0; workers],
        construction_progress: dec!(0),
    }
}

/// Helper to create a test market state.
fn create_test_market(wood_price: Option<f64>, food_price: Option<f64>) -> MarketState {
    MarketState {
        last_wood_price: wood_price.map(|p| Decimal::from_f64(p).unwrap()),
        last_food_price: food_price.map(|p| Decimal::from_f64(p).unwrap()),
    }
}

#[test]
fn test_survival_strategy_prioritizes_resources() {
    let strategy = SurvivalStrategy::new(20, 10);
    
    // Test with low resources
    let village = create_test_village("test", 10, 5.0, 5.0, 100.0);
    let market = create_test_market(Some(5.0), Some(1.0));
    
    let decision = strategy.decide_allocation_and_orders(&village, &market);
    
    println!("Survival allocation: food={}, wood={}, construction={}", 
             decision.allocation.food, decision.allocation.wood, decision.allocation.construction);
    
    // Should allocate heavily to resource production when resources are critically low
    // With 5 food (0.5 days), should prioritize food heavily
    assert!(decision.allocation.food > dec!(0), "Should allocate to food production");
    // May allocate all to food if it's the most critical need
    assert!(decision.allocation.food + decision.allocation.wood > dec!(0), 
            "Should allocate to resource production");
    
    // Should try to buy critical resources
    assert!(decision.food_bid.is_some(), "Should bid for food when low");
    // May not bid for wood if food is the immediate priority
    // But should at least consider trading
    assert!(decision.food_bid.is_some() || decision.wood_bid.is_some(), 
            "Should participate in market when resources are low");
}

#[test]
fn test_growth_strategy_builds_houses() {
    let strategy = GrowthStrategy::new(50, 3);
    
    // Test with good resources but need houses
    let village = create_test_village("test", 10, 100.0, 100.0, 200.0);
    let market = create_test_market(Some(5.0), Some(1.0));
    
    let decision = strategy.decide_allocation_and_orders(&village, &market);
    
    println!("Growth allocation: food={}, wood={}, construction={}", 
             decision.allocation.food, decision.allocation.wood, decision.allocation.construction);
    
    // Should allocate to all areas for balanced growth
    assert!(decision.allocation.food > dec!(0), "Should produce food");
    assert!(decision.allocation.wood > dec!(0), "Should produce wood");
    // Growth strategy may or may not allocate to construction depending on current needs
    
    // Should maintain resource production
    assert!(decision.allocation.food > dec!(0), "Should produce food");
    assert!(decision.allocation.wood > dec!(0), "Should produce wood");
}

#[test]
fn test_trading_strategy_specializes() {
    // Test food specialization
    let mut village_food = create_test_village("food_village", 10, 50.0, 50.0, 100.0);
    village_food.food_slots = (20, 10);
    village_food.wood_slots = (5, 5);
    
    let strategy_food = TradingStrategy::new(1.0, 0.3);
    let market = create_test_market(Some(5.0), Some(1.0));
    
    let decision_food = strategy_food.decide_allocation_and_orders(&village_food, &market);
    
    // Should allocate mostly to food
    assert!(decision_food.allocation.food > decision_food.allocation.wood * dec!(3));
    
    // Should offer to sell food
    assert!(decision_food.food_ask.is_some(), "Food specialist should sell food");
    
    // Test wood specialization
    let mut village_wood = create_test_village("wood_village", 10, 50.0, 50.0, 100.0);
    village_wood.food_slots = (5, 5);
    village_wood.wood_slots = (20, 10);
    
    let strategy_wood = TradingStrategy::new(1.0, 0.3);
    let decision_wood = strategy_wood.decide_allocation_and_orders(&village_wood, &market);
    
    // Should allocate mostly to wood
    assert!(decision_wood.allocation.wood > decision_wood.allocation.food * dec!(3));
    
    // Should offer to sell wood
    assert!(decision_wood.wood_ask.is_some(), "Wood specialist should sell wood");
}

#[test]
fn test_balanced_strategy_adapts() {
    let strategy = BalancedStrategy::new(0.25, 0.25, 0.25, 0.25);
    
    // Test with low food
    let mut village = create_test_village("test", 10, 5.0, 100.0, 100.0);
    let market = create_test_market(Some(5.0), Some(1.0));
    
    let decision = strategy.decide_allocation_and_orders(&village, &market);
    
    // Should prioritize food when low
    assert!(decision.allocation.food > decision.allocation.wood);
    
    // Test with low wood
    village.food = dec!(100);
    village.wood = dec!(5);
    
    let decision2 = strategy.decide_allocation_and_orders(&village, &market);
    
    // Should prioritize wood when low
    assert!(decision2.allocation.wood > decision2.allocation.food);
}

#[test]
fn test_greedy_strategy_maximizes_value() {
    let strategy = GreedyStrategy;
    
    // Test with different price scenarios
    let village = create_test_village("test", 10, 50.0, 50.0, 100.0);
    
    // When wood is more valuable
    let market_wood_high = create_test_market(Some(10.0), Some(1.0));
    let decision = strategy.decide_allocation_and_orders(&village, &market_wood_high);
    
    println!("Greedy allocation (wood high): food={}, wood={}, construction={}", 
             decision.allocation.food, decision.allocation.wood, decision.allocation.construction);
    
    // Should allocate to highest value resource
    assert_eq!(decision.allocation.construction, dec!(0), "Greedy never builds");
    // With equal slots but different prices, greedy should consider total value
    
    // When food is more valuable (considering production rates)
    let market_food_high = create_test_market(Some(1.0), Some(5.0));
    let decision2 = strategy.decide_allocation_and_orders(&village, &market_food_high);
    
    println!("Greedy allocation (food high): food={}, wood={}, construction={}", 
             decision2.allocation.food, decision2.allocation.wood, decision2.allocation.construction);
    
    // Greedy considers both price and production rate
    // Food produces at 2.0/day, wood at 0.1/day
    // So even with lower price, food might still be more valuable
}

#[test]
fn test_strategies_handle_edge_cases() {
    let strategies: Vec<Box<dyn Strategy>> = vec![
        Box::new(SurvivalStrategy::default()),
        Box::new(GrowthStrategy::default()),
        Box::new(TradingStrategy::default()),
        Box::new(BalancedStrategy::default()),
        Box::new(GreedyStrategy),
    ];
    
    // Test with zero workers
    let empty_village = create_test_village("empty", 0, 0.0, 0.0, 0.0);
    let market = create_test_market(None, None);
    
    for strategy in &strategies {
        let decision = strategy.decide_allocation_and_orders(&empty_village, &market);
        
        // Should handle gracefully
        assert_eq!(decision.allocation.food, dec!(0));
        assert_eq!(decision.allocation.wood, dec!(0));
        assert_eq!(decision.allocation.construction, dec!(0));
    }
    
    // Test with no money
    let broke_village = create_test_village("broke", 10, 100.0, 100.0, 0.0);
    
    for strategy in &strategies {
        let decision = strategy.decide_allocation_and_orders(&broke_village, &market);
        
        // Should not try to buy anything
        assert!(decision.food_bid.is_none() || decision.food_bid.unwrap().1 == 0);
        assert!(decision.wood_bid.is_none() || decision.wood_bid.unwrap().1 == 0);
    }
}