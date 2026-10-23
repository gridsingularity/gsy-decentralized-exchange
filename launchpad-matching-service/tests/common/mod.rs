//! Shared helpers for the matching golden test and the matching benchmark.
//!
//! `reference` is a verbatim copy of the Pay-As-Bid implementation and the
//! per-(market, time slot) grouping as they were before MS-10, kept as the
//! reference the optimized implementation is compared against. `rng` is a
//! tiny seeded random number generator, so no new dependency is needed.
//! `payload` builds `POST /match` bodies of a given size.

#![allow(dead_code)]

pub mod payload;
pub mod reference;
pub mod rng;
