#[cfg(test)]
mod tests {
    use super::super::events::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_creation_and_display() {
        let event = Event {
            timestamp: Utc::now(),
            tick: 10,
            village_id: "test_village".to_string(),
            event_type: EventType::ResourceProduced {
                resource: ResourceType::Food,
                amount: dec!(5.0),
                workers_assigned: 2,
            },
        };

        let display = format!("{}", event);
        assert!(display.contains("[10] Village test_village:"));
        assert!(display.contains("Produced 5.0 Food with 2 workers"));
    }

    #[test]
    fn test_event_logger() {
        let mut logger = EventLogger::new();

        logger.log(
            1,
            "village_a".to_string(),
            EventType::WorkerBorn {
                worker_id: 1,
                total_population: 11,
            },
        );

        logger.log(
            2,
            "village_a".to_string(),
            EventType::ResourceConsumed {
                resource: ResourceType::Food,
                amount: dec!(10.0),
                purpose: ConsumptionPurpose::WorkerFeeding,
            },
        );

        let events = logger.get_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].tick, 1);
        assert_eq!(events[1].tick, 2);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            timestamp: Utc::now(),
            tick: 5,
            village_id: "test".to_string(),
            event_type: EventType::TradeExecuted {
                resource: ResourceType::Wood,
                quantity: dec!(10.0),
                price: dec!(2.5),
                counterparty: "other_village".to_string(),
                side: TradeSide::Buy,
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(event.tick, deserialized.tick);
        assert_eq!(event.village_id, deserialized.village_id);
    }

    #[test]
    fn test_event_logger_persistence() {
        let mut logger = EventLogger::new();

        logger.log(
            1,
            "v1".to_string(),
            EventType::VillageStateSnapshot {
                population: 10,
                houses: 2,
                food: dec!(50.0),
                wood: dec!(40.0),
                money: dec!(100.0),
            },
        );

        let temp_file = "/tmp/test_events.json";
        logger.save_to_file(temp_file).unwrap();

        let loaded_logger = EventLogger::load_from_file(temp_file).unwrap();
        assert_eq!(loaded_logger.get_events().len(), 1);

        std::fs::remove_file(temp_file).ok();
    }
}
