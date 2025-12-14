//! Parsnip Search - Search engines for knowledge graph
//!
//! Provides exact search, fuzzy search (nucleo), full-text search (tantivy), vector search.

pub mod error;
pub mod exact;
pub mod traits;

#[cfg(feature = "fuzzy")]
pub mod fuzzy;

#[cfg(feature = "fulltext")]
pub mod fulltext;

#[cfg(feature = "fulltext")]
pub mod hybrid;

#[cfg(feature = "vector")]
pub mod vector;

pub use error::{SearchError, SearchResult};
pub use exact::ExactSearchEngine;
pub use traits::{SearchEngine, SearchHit};

#[cfg(feature = "fuzzy")]
pub use fuzzy::FuzzySearchEngine;

#[cfg(feature = "fulltext")]
pub use fulltext::FullTextSearchEngine;

#[cfg(feature = "fulltext")]
pub use hybrid::HybridSearchEngine;

#[cfg(feature = "vector")]
pub use vector::VectorSearchEngine;
