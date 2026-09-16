//! A small, current-engine terminal advisor.
//!
//! This first vertical slice deliberately uses a bounded one-round search. It is separate
//! from the frozen historical solver and refuses draws outside the strict catalog projection.

pub mod input;
pub mod search;
pub mod view;
