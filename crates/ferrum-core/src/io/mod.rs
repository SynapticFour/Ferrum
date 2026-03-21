//! Blocking I/O helpers (POSIX / HPC-oriented).
//!
//! Lesson: dedicated thread pool for blocking filesystem work.
//! Source: Tokio docs (async fs uses internal pool); Rust users forum / HPC experience with glibc.
//! Reason: avoid competing with Tokio's blocking pool under high concurrent POSIX read load.

pub mod posix;
