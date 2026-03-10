//! Infrastructure-as-Code: declarative config management via plan/apply/export.
//!
//! This module provides a CLI-driven GitOps workflow for TrueFlow configuration.
//! It talks to the gateway REST API (not the database directly) so it works
//! remotely and respects all auth/validation logic.

pub mod client;
pub mod diff;
pub mod schema;
