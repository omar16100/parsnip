//! Input validation limits for security and resource protection

/// Maximum length for entity names (256 chars)
pub const MAX_ENTITY_NAME_LEN: usize = 256;

/// Maximum length for a single observation (64KB)
pub const MAX_OBSERVATION_LEN: usize = 64 * 1024;

/// Maximum observations per entity (1000)
pub const MAX_OBSERVATIONS_PER_ENTITY: usize = 1000;

/// Maximum entities in a batch create (100)
pub const MAX_BATCH_ENTITIES: usize = 100;

/// Maximum relations in a batch create (100)
pub const MAX_BATCH_RELATIONS: usize = 100;

/// Maximum traversal depth (50)
pub const MAX_TRAVERSAL_DEPTH: u32 = 50;

/// Maximum nodes in a single traversal result (10000)
pub const MAX_TRAVERSAL_NODES: usize = 10000;

/// Maximum tags per entity (100)
pub const MAX_TAGS_PER_ENTITY: usize = 100;

/// Maximum tag length (64 chars)
pub const MAX_TAG_LEN: usize = 64;

/// Maximum project name length (64 chars)
pub const MAX_PROJECT_NAME_LEN: usize = 64;

/// Validation error type
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    EntityNameTooLong { len: usize, max: usize },
    ObservationTooLong { len: usize, max: usize },
    TooManyObservations { count: usize, max: usize },
    TooManyEntities { count: usize, max: usize },
    TooManyRelations { count: usize, max: usize },
    TraversalDepthTooLarge { depth: u32, max: u32 },
    TooManyTags { count: usize, max: usize },
    TagTooLong { len: usize, max: usize },
    ProjectNameTooLong { len: usize, max: usize },
    EmptyEntityName,
    EmptyObservation,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EntityNameTooLong { len, max } => {
                write!(f, "Entity name too long: {} chars (max {})", len, max)
            }
            Self::ObservationTooLong { len, max } => {
                write!(f, "Observation too long: {} chars (max {})", len, max)
            }
            Self::TooManyObservations { count, max } => {
                write!(f, "Too many observations: {} (max {})", count, max)
            }
            Self::TooManyEntities { count, max } => {
                write!(f, "Too many entities in batch: {} (max {})", count, max)
            }
            Self::TooManyRelations { count, max } => {
                write!(f, "Too many relations in batch: {} (max {})", count, max)
            }
            Self::TraversalDepthTooLarge { depth, max } => {
                write!(f, "Traversal depth too large: {} (max {})", depth, max)
            }
            Self::TooManyTags { count, max } => {
                write!(f, "Too many tags: {} (max {})", count, max)
            }
            Self::TagTooLong { len, max } => {
                write!(f, "Tag too long: {} chars (max {})", len, max)
            }
            Self::ProjectNameTooLong { len, max } => {
                write!(f, "Project name too long: {} chars (max {})", len, max)
            }
            Self::EmptyEntityName => write!(f, "Entity name cannot be empty"),
            Self::EmptyObservation => write!(f, "Observation cannot be empty"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate entity name
pub fn validate_entity_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::EmptyEntityName);
    }
    if name.len() > MAX_ENTITY_NAME_LEN {
        return Err(ValidationError::EntityNameTooLong {
            len: name.len(),
            max: MAX_ENTITY_NAME_LEN,
        });
    }
    Ok(())
}

/// Validate observation
pub fn validate_observation(obs: &str) -> Result<(), ValidationError> {
    if obs.is_empty() {
        return Err(ValidationError::EmptyObservation);
    }
    if obs.len() > MAX_OBSERVATION_LEN {
        return Err(ValidationError::ObservationTooLong {
            len: obs.len(),
            max: MAX_OBSERVATION_LEN,
        });
    }
    Ok(())
}

/// Validate tag
pub fn validate_tag(tag: &str) -> Result<(), ValidationError> {
    if tag.len() > MAX_TAG_LEN {
        return Err(ValidationError::TagTooLong {
            len: tag.len(),
            max: MAX_TAG_LEN,
        });
    }
    Ok(())
}

/// Validate batch entity count
pub fn validate_batch_entities(count: usize) -> Result<(), ValidationError> {
    if count > MAX_BATCH_ENTITIES {
        return Err(ValidationError::TooManyEntities {
            count,
            max: MAX_BATCH_ENTITIES,
        });
    }
    Ok(())
}

/// Validate batch relation count
pub fn validate_batch_relations(count: usize) -> Result<(), ValidationError> {
    if count > MAX_BATCH_RELATIONS {
        return Err(ValidationError::TooManyRelations {
            count,
            max: MAX_BATCH_RELATIONS,
        });
    }
    Ok(())
}

/// Validate traversal depth
pub fn validate_traversal_depth(depth: u32) -> Result<(), ValidationError> {
    if depth > MAX_TRAVERSAL_DEPTH {
        return Err(ValidationError::TraversalDepthTooLarge {
            depth,
            max: MAX_TRAVERSAL_DEPTH,
        });
    }
    Ok(())
}

/// Validate project name
pub fn validate_project_name(name: &str) -> Result<(), ValidationError> {
    if name.len() > MAX_PROJECT_NAME_LEN {
        return Err(ValidationError::ProjectNameTooLong {
            len: name.len(),
            max: MAX_PROJECT_NAME_LEN,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_entity_name() {
        assert!(validate_entity_name("valid_name").is_ok());
        assert!(validate_entity_name("").is_err());
        assert!(validate_entity_name(&"x".repeat(300)).is_err());
    }

    #[test]
    fn test_validate_observation() {
        assert!(validate_observation("valid obs").is_ok());
        assert!(validate_observation("").is_err());
        assert!(validate_observation(&"x".repeat(100_000)).is_err());
    }
}
