//! sched.rs — the scheduling policy. (Exercise 06 reference solution.)

use crate::proc::ProcState;

pub trait Scheduler {
    fn pick_next(&mut self, states: &[ProcState]) -> Option<usize>;
}

pub struct RoundRobin {
    next: usize,
}

impl RoundRobin {
    pub const fn new() -> RoundRobin {
        RoundRobin { next: 0 }
    }
}

impl Scheduler for RoundRobin {
    fn pick_next(&mut self, states: &[ProcState]) -> Option<usize> {
        let n = states.len();
        (0..n)
            .map(|off| (self.next + off) % n)
            .find(|&i| states[i] == ProcState::Runnable)
            .map(|i| {
                self.next = (i + 1) % n;
                i
            })
    }
}
