#[cfg(test)]
mod tests {
    use super::super::events::*;
    use super::super::metrics::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn create_test_events() -> Vec<Event> {
        let mut events = vec![];
        let base_time = Utc::now();

        events.push(Event {
            timestamp: base_time,
            tick: 0,
            village_id: "test_village".to_string(),
            event_type: EventType::VillageStateSnapshot {
                population: 10,
                houses: 2,
                food: dec!(50.0),
                wood: dec!(50.0),
                money: dec!(100.0),
            },
        });

        events.push(Event {
            timestamp: base_time,
            tick: 1,
            village_id: "test_village".to_string(),
            event_type: EventType::ResourceProduced {
                resource: ResourceType::Food,
                amount: dec!(10.0),
                workers_assigned: 2,
            },
        });

        events.push(Event {
            timestamp: base_time,
            tick: 2,
            village_id: "test_village".to_string(),
            event_type: EventType::WorkerBorn {
                worker_id: 11,
                total_population: 11,
            },
        });

        events.push(Event {
            timestamp: base_time,
            tick: 3,
            village_id: "test_village".to_string(),
            event_type: EventType::WorkerDied {
                worker_id: 5,
                cause: DeathCause::Starvation,
                total_population: 10,
            },
        });

        events.push(Event {
            timestamp: base_time,
            tick: 10,
            village_id: "test_village".to_string(),
            event_type: EventType::VillageStateSnapshot {
                population: 10,
                houses: 2,
                food: dec!(45.0),
                wood: dec!(48.0),
                money: dec!(95.0),
            },
        });

        events
    }

    #[test]
    fn test_village_metrics_calculation() {
        let events = create_test_events();
        let metrics = MetricsCalculator::calculate_village_metrics("test_village", &events, 10, 10);

        assert_eq!(metrics.village_id, "test_village");
        assert_eq!(metrics.final_population, 10);
        assert_eq!(metrics.total_births, 1);
        assert_eq!(metrics.total_deaths, 1);
        assert_eq!(metrics.starvation_deaths, 1);
        assert_eq!(metrics.days_survived, 10);
        assert_eq!(metrics.survival_score, 1.0);
    }

    #[test]
    fn test_scenario_metrics() {
        let mut events = create_test_events();

        for event in &mut events[..3] {
            event.village_id = "village_a".to_string();
        }

        events.push(Event {
            timestamp: Utc::now(),
            tick: 1,
            village_id: "village_b".to_string(),
            event_type: EventType::VillageStateSnapshot {
                population: 5,
                houses: 1,
                food: dec!(20.0),
                wood: dec!(20.0),
                money: dec!(50.0),
            },
        });

        let village_configs = vec![("village_a".to_string(), 10), ("village_b".to_string(), 5)];

        let metrics = MetricsCalculator::calculate_scenario_metrics(&events, &village_configs, 10);

        assert_eq!(metrics.villages.len(), 2);
        assert!(metrics.aggregate_survival_rate > 0.0);
    }

    #[test]
    fn test_gini_coefficient() {
        let values = vec![10.0, 10.0, 10.0, 10.0];
        let gini = MetricsCalculator::calculate_gini_coefficient(&values);
        assert!(gini < 0.01);

        let values = vec![100.0, 0.0, 0.0, 0.0];
        let gini = MetricsCalculator::calculate_gini_coefficient(&values);
        assert!(gini > 0.7);
    }

    #[test]
    fn test_metrics_display() {
        let metrics = VillageMetrics {
            village_id: "test".to_string(),
            survival_score: 0.8,
            growth_score: 0.5,
            economic_efficiency: 1.2,
            trade_effectiveness: 0.3,
            stability_score: 0.9,
            overall_score: 0.7,
            initial_population: 10,
            final_population: 8,
            peak_population: 12,
            total_births: 3,
            total_deaths: 5,
            starvation_deaths: 2,
            shelter_deaths: 3,
            total_food_produced: dec!(100.0),
            total_wood_produced: dec!(80.0),
            total_food_consumed: dec!(90.0),
            total_wood_consumed: dec!(70.0),
            houses_built: 2,
            final_houses: 3,
            average_house_maintenance: dec!(0.8),
            trades_executed: 10,
            trade_volume: dec!(50.0),
            trade_profit: dec!(15.0),
            days_survived: 100,
            population_variance: 2.5,
        };

        let display = format!("{}", metrics);
        assert!(display.contains("Village test Metrics:"));
        assert!(display.contains("Overall Score: 0.70"));
    }
}
