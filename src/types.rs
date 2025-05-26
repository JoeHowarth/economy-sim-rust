use rust_decimal::Decimal;

// Re-export ResourceType from events module
pub use crate::events::ResourceType;

// Extension methods for ResourceType
pub trait ResourceTypeExt {
    /// Get the string identifier for auction system
    fn as_str(&self) -> &'static str;
    
    /// Parse from string
    fn from_str(s: &str) -> Option<ResourceType>;
}

impl ResourceTypeExt for ResourceType {
    fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Wood => "wood",
            ResourceType::Food => "food",
        }
    }
    
    fn from_str(s: &str) -> Option<ResourceType> {
        match s {
            "wood" => Some(ResourceType::Wood),
            "food" => Some(ResourceType::Food),
            _ => None,
        }
    }
}

/// A request to place an order in the market
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub resource: ResourceType,
    pub is_buy: bool,
    pub quantity: u32,
    pub price: Decimal,
}

/// Unique identifier for a village
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VillageId(pub String);

impl VillageId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    
    /// Get numeric ID for auction participant
    /// This provides a consistent way to map village IDs to participant IDs
    pub fn to_participant_id(&self) -> u32 {
        // Use a better hash function for consistency
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish() as u32
    }
}