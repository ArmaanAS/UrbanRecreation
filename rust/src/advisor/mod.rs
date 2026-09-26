//! A small, current-engine terminal advisor.
//!
//! This vertical slice combines an exact continuation policy for every round, the opening
//! included, with a manual multi-round session. It is separate from the frozen historical solver and
//! refuses draws outside the strict catalog projection.

pub mod input;
pub mod jsonl;
pub mod policy;
pub mod search;
pub mod session;
pub mod view;
