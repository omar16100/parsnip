//! Observation types - facts stored about entities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Unique identifier for an observation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(pub Ulid);

impl ObservationId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for ObservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ObservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An observation (fact) about an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Unique identifier
    pub id: ObservationId,

    /// The observation content
    pub content: String,

    /// Optional source of this observation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Confidence score (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// When this observation was created
    pub created_at: DateTime<Utc>,
}

impl Observation {
    /// Create a new observation
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: ObservationId::new(),
            content: content.into(),
            source: None,
            confidence: None,
            created_at: Utc::now(),
        }
    }

    /// Create observation with source
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Create observation with confidence
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_creation() {
        let obs = Observation::new("Works at Google");
        assert_eq!(obs.content, "Works at Google");
        assert!(obs.source.is_none());
        assert!(obs.confidence.is_none());
    }

    #[test]
    fn test_observation_with_metadata() {
        let obs = Observation::new("Expert in Rust")
            .with_source("LinkedIn profile")
            .with_confidence(0.9);

        assert_eq!(obs.source, Some("LinkedIn profile".to_string()));
        assert_eq!(obs.confidence, Some(0.9));
    }

    #[test]
    fn test_confidence_clamping() {
        let obs = Observation::new("Test").with_confidence(1.5);
        assert_eq!(obs.confidence, Some(1.0));

        let obs = Observation::new("Test").with_confidence(-0.5);
        assert_eq!(obs.confidence, Some(0.0));
    }
}
