//! A small, current-engine terminal advisor.
//!
//! This vertical slice combines a bounded early-game estimate, exact late-game policy, and
//! a manual multi-round session. It is separate from the frozen historical solver and
//! refuses draws outside the strict catalog projection.

pub mod input;
pub mod policy;
pub mod search;
pub mod session;
pub mod view;
