//! semaphore.rs — a counting semaphore. (Exercise 08 reference solution.)

use crate::spinlock::SpinLock;

pub struct Semaphore {
    count: SpinLock<i64>,
}

impl Semaphore {
    pub fn new(permits: i64) -> Semaphore {
        Semaphore {
            count: SpinLock::new(permits),
        }
    }

    pub fn try_wait(&self) -> bool {
        let mut count = self.count.lock();
        if *count > 0 {
            *count -= 1;
            true
        } else {
            false
        }
    }

    pub fn post(&self) {
        let mut count = self.count.lock();
        *count += 1;
    }

    pub fn available(&self) -> i64 {
        *self.count.lock()
    }
}
