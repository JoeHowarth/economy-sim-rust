pub mod analysis;
pub mod auction;
pub mod auction_builder;
pub mod batch_analysis;
pub mod cli;
pub mod core;
pub mod events;
pub mod experiment;
pub mod metrics;
pub mod query;
pub mod scenario;
pub mod strategies;
pub mod types;
pub mod ui;
pub mod visualization;

#[cfg(test)]
mod events_test;
#[cfg(test)]
mod metrics_test;
#[cfg(test)]
mod scenario_test;
