//! sched.rs — the scheduling *policy*: deciding which process runs next.
//!
//! The mechanism for running a process (saving/restoring registers) is `swtch`
//! from exercise 05. This file is the *policy* layered on top: given the state
//! of every process, which one gets the CPU next?

use crate::proc::ProcState;

/// A scheduling policy.
///
/// This is a **trait** — a set of methods a type promises to provide, like an
/// interface. Any number of policies (round-robin, priority, lottery, ...) can
/// `impl Scheduler`, and the scheduler loop can drive any of them through this
/// one shared method. That decoupling is exactly what traits are for.
pub trait Scheduler {
    /// Given the current `state` of each process slot, return the index of the
    /// slot to run next, or `None` if nothing is `Runnable`.
    fn pick_next(&mut self, states: &[ProcState]) -> Option<usize>;
}

/// Round-robin: give each Runnable process a turn in order, then wrap around
/// and start again — so no process is starved while others keep running.
pub struct RoundRobin {
    /// The slot index to begin scanning from on the next pick (one past the
    /// slot we chose last time).
    next: usize,
}

impl RoundRobin {
    pub const fn new() -> RoundRobin {
        RoundRobin { next: 0 }
    }
}

impl Scheduler for RoundRobin {
    fn pick_next(&mut self, states: &[ProcState]) -> Option<usize> {
        // IMPLEMENT: round-robin selection.
        //
        //   Let n = states.len(). Examine the slots in order starting at
        //   `self.next` and wrapping around the end — that is, the indices
        //       (self.next + 0) % n, (self.next + 1) % n, ... , (self.next + n-1) % n
        //   Return the FIRST one whose state is `ProcState::Runnable`:
        //       - before returning index i, set self.next = (i + 1) % n so the
        //         next call resumes *after* it (this is what makes it rotate);
        //       - return Some(i).
        //   If none of the slots are Runnable, return None.
        //
        //   Tip: this is a natural fit for an iterator chain — build the
        //   candidate indices from a range with `.map(...)`, then `.find(...)`
        //   the first Runnable one. Don't forget to update `self.next`.
        let _ = states; // remove once implemented
        None
    }
}
