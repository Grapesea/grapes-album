//! Deterministic classification helpers.
//!
//! These classifiers intentionally avoid models: `time` derives the light band
//! from the sun's position and `tone` derives temperature/saturation from
//! CIELAB statistics of the cached thumbnail. Every result records its method,
//! prompt version and evidence so it can be compared with, or rolled back in
//! favour of, a later model-based pass.

pub mod time;
pub mod tone;

pub use time::{classify_time, TimeBand};
pub use tone::classify_tone;

pub const TIME_DIMENSION_ID: &str = "dimension:time";
pub const TONE_DIMENSION_ID: &str = "dimension:tone";

/// One deterministic classification proposal for a single asset.
///
/// The dimension and value identifiers are the stable seed identifiers created
/// by `005_classifications.sql`; rows are written with `source = 'derived'`.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedClassification {
    pub dimension_id: &'static str,
    pub dimension_name: &'static str,
    pub value_id: &'static str,
    pub value_name: &'static str,
    pub confidence: f64,
    pub method: &'static str,
    pub prompt_version: &'static str,
    pub evidence: String,
}
