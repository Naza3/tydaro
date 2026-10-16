//! Composable GPUI controls shared by application views and component examples.
//!
//! This crate owns appearance and interaction primitives, never application data,
//! navigation, persistence, service initialization, or window-local input state.
//! Callers supply stable element IDs, own their input entities, and attach their
//! event handlers. Returned GPUI elements remain configurable by their callers.

pub mod action;
pub mod feedback;
pub mod field;
pub mod selection;
pub mod surface;
pub mod text;
pub mod theme;
pub mod tokens;
