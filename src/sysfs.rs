//! Linux `sysfs` walker.

use crate::{common::ThreadHandle, early_logging::KConsole, module::ModLoading, PROGRAM_NAME};
use mio::Waker;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    sync::{mpsc::channel, Arc},
    thread,
};

mod modalias {
    use crate::module::ModLoading;
    use mio::Waker;
    use precisej_printable_errno::PrintableErrno;
    use std::sync::{mpsc::Sender, Arc};

    /// Function called when the `sysfs` modalias thread is spawned.
    pub(super) fn spawn(
        main_waker: Arc<Waker>,
        tx_udev_waker: Sender<Result<Arc<Waker>, PrintableErrno<String>>>,
        mod_loading: ModLoading,
    ) {
        todo!()
    }
}

mod walker {
    use mio::Waker;
    use precisej_printable_errno::PrintableErrno;
    use std::sync::{mpsc::Sender, Arc};

    /// Function called when the `sysfs` walker thread is spawned.
    pub(super) fn spawn(
        main_waker: Arc<Waker>,
        tx_udev_waker: Sender<Result<Arc<Waker>, PrintableErrno<String>>>,
    ) {
        todo!()
    }
}

#[derive(Debug)]
pub struct SysfsWalker {
    modaliases_t: ThreadHandle,
    block_t: ThreadHandle,
}
impl SysfsWalker {
    /// Construct the `sysfs`-walking threads which will notify when `/system_root` is mounted.
    pub fn walk(
        main_waker: &Arc<Waker>,
        mod_loading: &ModLoading,
    ) -> Result<Self, PrintableErrno<String>> {
        let modaliases_t = {
            let main_waker_cl = Arc::clone(main_waker);
            let (tx_mod_waker, rx_mod_waker) = channel();
            let mod_loading = mod_loading.clone();
            let mod_handle = thread::spawn(move || {
                modalias::spawn(main_waker_cl, tx_mod_waker, mod_loading)
            });
            let mod_waker = rx_mod_waker.recv().map_err(|e| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while spawning sysfs-modalias thread: {}", e),
                )
            })??;
            ThreadHandle::new("sysfs-modalias", mod_handle, mod_waker)
        };

        let block_t = {
            let main_waker_cl = Arc::clone(main_waker);
            let (tx_walk_waker, rx_walk_waker) = channel();
            let walk_handle = thread::spawn(move || walker::spawn(main_waker_cl, tx_walk_waker));
            let walk_waker = rx_walk_waker.recv().map_err(|e| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while spawning sysfs-walker thread: {}", e),
                )
            })??;
            ThreadHandle::new("sysfs-walker", walk_handle, walk_waker)
        };

        Ok(Self {
            modaliases_t,
            block_t,
        })
    }

    /// Stop the `sysfs`-walking threads and cleanup.
    pub fn stop(self, kcon: &mut KConsole) {
        self.block_t.join_now(kcon);
        self.modaliases_t.join_now(kcon);
    }
}
