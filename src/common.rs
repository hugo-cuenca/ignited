//! Common structs and functions used by various (rust code) modules.

use crate::{early_logging::KConsole, PROGRAM_NAME};
use mio::Waker;
use precisej_printable_errno::printable_error;
use std::{sync::Arc, thread::JoinHandle};

pub struct ThreadHandle {
    name: &'static str,
    join_t: JoinHandle<()>,
    waker_t: Arc<Waker>,
}
impl ThreadHandle {
    /// Construct a new thread.
    pub fn new(name: &'static str, join_t: JoinHandle<()>, waker_t: Arc<Waker>) -> ThreadHandle {
        ThreadHandle {
            name,
            join_t,
            waker_t,
        }
    }

    /// Stop the thread and cleanup.
    pub fn join_now(self, kmsg: &mut KConsole) {
        if let Err(e) = self.waker_t.wake().map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("FATAL: error while notifying {} to stop: {}", self.name, io),
            )
        }) {
            kcrit!(kmsg, "{}", e);
        }

        let _ = self.join_t.join();
    }
}
