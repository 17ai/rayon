#![allow(non_camel_case_types)] // I prefer to use ALL_CAPS for type parameters
#![cfg_attr(test, feature(conservative_impl_trait))]

// If you're not compiling the unstable code, it often happens that
// there is stuff that is considered "dead code" and so forth. So
// disable warnings in that scenario.
#![cfg_attr(not(feature = "unstable"), allow(warnings))]

extern crate deque;
#[cfg(feature = "unstable")]
extern crate futures;
extern crate libc;
extern crate num_cpus;
extern crate rand;
extern crate rayon_core;

pub mod par_iter;
pub mod prelude;
pub mod string;
mod test;

pub use rayon_core::current_num_threads;
pub use rayon_core::Configuration;
pub use rayon_core::PanicHandler;
pub use rayon_core::InitError;
pub use rayon_core::dump_stats;
pub use rayon_core::initialize;
pub use rayon_core::ThreadPool;
pub use rayon_core::join;
pub use rayon_core::{scope, Scope};
#[cfg(feature = "unstable")]
pub use rayon_core::spawn_async;
#[cfg(feature = "unstable")]
pub use rayon_core::spawn_future_async;
#[cfg(feature = "unstable")]
pub use rayon_core::RayonFuture;
