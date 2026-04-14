//! JSON-RPC method handler modules
//!
//! Each module registers its methods via `inventory::submit!`.
//! The central dispatch in `handler.rs` discovers them automatically.

pub mod auth;
pub mod contact;
pub mod entry;
pub mod file;
pub mod lifecycle;
pub mod quota;
pub mod search;
pub mod space;
pub mod team;
