//! Remoting for [`StorageBackend`](crate::traits::StorageBackend).
//!
//! One process owns the database; every other consumer reaches it over HTTP. redb takes
//! an exclusive process-wide lock, so without this a running `parsnip serve` makes every
//! local CLI invocation fail with "Database already open".
//!
//! - [`protocol`] defines the wire format shared by both ends.
//! - [`dispatch`] is the server side: run a decoded call against a local backend.
//! - `client` (feature `remote`) is the client side: a `StorageBackend` implementation
//!   that forwards each method over HTTP.
//!
//! Encoder and decoder live in one directory so they cannot drift apart, and the whole
//! protocol is testable without a socket by looping the client into the dispatcher.

pub mod dispatch;
pub mod protocol;

#[cfg(feature = "remote")]
pub mod client;

pub use dispatch::StorageDispatcher;
pub use protocol::{method, GraphPayload, WireError, STORAGE_ERROR_CODE};

#[cfg(feature = "remote")]
pub use client::{HttpTransport, RemoteStorage, RpcTransport};
