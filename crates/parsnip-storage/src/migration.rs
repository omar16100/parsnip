//! Schema migrations for Parsnip storage backends
//!
//! Provides version tracking and migration functions for schema changes.

use crate::StorageResult;

/// Current schema version
pub const CURRENT_VERSION: u32 = 1;

/// Schema migration information
#[derive(Debug, Clone)]
pub struct SchemaVersion {
    pub version: u32,
    pub description: &'static str,
}

/// All schema versions with their migrations
pub fn get_migrations() -> Vec<SchemaVersion> {
    vec![
        SchemaVersion {
            version: 1,
            description: "Initial schema with entities, relations, and projects",
        },
    ]
}

/// Migration trait for storage backends
pub trait Migratable {
    /// Get the current schema version from storage
    fn get_schema_version(&self) -> StorageResult<u32>;

    /// Set the schema version in storage
    fn set_schema_version(&self, version: u32) -> StorageResult<()>;

    /// Run migrations from current version to target version
    fn migrate_to(&self, target_version: u32) -> StorageResult<()> {
        let current = self.get_schema_version()?;

        if current == target_version {
            tracing::debug!("Schema already at version {}", target_version);
            return Ok(());
        }

        if current > target_version {
            tracing::warn!(
                "Schema version {} is newer than target {}. Downgrades not supported.",
                current,
                target_version
            );
            return Ok(());
        }

        tracing::info!("Migrating schema from v{} to v{}", current, target_version);

        for version in (current + 1)..=target_version {
            self.run_migration(version)?;
            self.set_schema_version(version)?;
            tracing::info!("Migrated to schema version {}", version);
        }

        Ok(())
    }

    /// Run a specific migration
    fn run_migration(&self, version: u32) -> StorageResult<()>;

    /// Migrate to the latest version
    fn migrate_to_latest(&self) -> StorageResult<()> {
        self.migrate_to(CURRENT_VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_list() {
        let migrations = get_migrations();
        assert!(!migrations.is_empty());
        assert_eq!(migrations[0].version, 1);
    }

    #[test]
    fn test_current_version() {
        assert_eq!(CURRENT_VERSION, 1);
    }
}
