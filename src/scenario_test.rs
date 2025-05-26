#[cfg(test)]
mod tests {
    use super::super::scenario::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_scenario_creation() {
        let mut scenario = Scenario::new("test_scenario".to_string());
        scenario.description = "A test scenario".to_string();

        let village = VillageConfig {
            id: "village_1".to_string(),
            initial_workers: 10,
            initial_houses: 2,
            initial_food: dec!(50.0),
            initial_wood: dec!(50.0),
            initial_money: dec!(100.0),
            food_slots: (10, 10),
            wood_slots: (10, 10),
            strategy: StrategyConfig::default(),
        };

        scenario.add_village(village);

        assert_eq!(scenario.villages.len(), 1);
        assert_eq!(scenario.name, "test_scenario");
    }

    #[test]
    fn test_scenario_validation() {
        let mut scenario = Scenario::new("invalid".to_string());

        assert!(scenario.validate().is_err());

        scenario.add_village(VillageConfig {
            id: "no_workers".to_string(),
            initial_workers: 0,
            initial_houses: 1,
            initial_food: dec!(10.0),
            initial_wood: dec!(10.0),
            initial_money: dec!(10.0),
            food_slots: (1, 1),
            wood_slots: (1, 1),
            strategy: StrategyConfig::default(),
        });

        assert!(scenario.validate().is_err());

        scenario.villages[0].initial_workers = 5;
        assert!(scenario.validate().is_ok());
    }

    #[test]
    fn test_scenario_serialization() {
        let scenario = create_standard_scenarios().get("basic").unwrap().clone();

        let json = serde_json::to_string_pretty(&scenario).unwrap();
        let deserialized: Scenario = serde_json::from_str(&json).unwrap();

        assert_eq!(scenario.name, deserialized.name);
        assert_eq!(scenario.villages.len(), deserialized.villages.len());
    }

    #[test]
    fn test_scenario_display() {
        let scenarios = create_standard_scenarios();
        let scenario = scenarios.get("basic").unwrap();
        let display = format!("{}", scenario);

        assert!(display.contains("Scenario: basic_two_villages"));
        assert!(display.contains("village_a"));
        assert!(display.contains("village_b"));
    }

    #[test]
    fn test_strategy_configs() {
        let strategies = vec![
            StrategyConfig::Balanced {
                food_weight: 0.3,
                wood_weight: 0.3,
                construction_weight: 0.2,
                repair_weight: 0.2,
            },
            StrategyConfig::Survival {
                min_food_days: 20,
                min_shelter_buffer: 3,
            },
            StrategyConfig::Growth {
                target_population: 100,
                house_buffer: 5,
            },
            StrategyConfig::Trading {
                price_multiplier: 1.5,
                max_trade_fraction: 0.3,
            },
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let _deserialized: StrategyConfig = serde_json::from_str(&json).unwrap();
        }
    }
}
