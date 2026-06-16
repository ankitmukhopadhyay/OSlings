//! param.rs — kernel configuration constants. (UNDERSTAND — you'll add more
//! of these as the kernel grows.)

/// The maximum number of processes the kernel can have at once. The process
/// table is a fixed-size array of this length — kernels avoid unbounded,
/// heap-grown structures in core data paths.
pub const NPROC: usize = 64;
