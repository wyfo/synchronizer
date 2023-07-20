#![forbid(clippy::dbg_macro)]
#![forbid(clippy::semicolon_if_nothing_returned)]
#![forbid(missing_docs)]
//! Synchronizer

mod runtime;
/// [`std::sync`] mock.
pub mod sync;
mod synchronizer;
/// [`std::thread`] mock.
pub mod thread;

pub use crate::synchronizer::{print_execution, run};

/// Add synchronization point in execution.
#[inline]
#[track_caller]
pub fn synchronize() {
    synchronizer::synchronized(());
}
