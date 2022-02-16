//! Miscellaneous functions that don't fit in any other (rust code) module.

use crate::PROGRAM_NAME;
use nix::{
    errno::Errno,
    sys::{stat::Mode, utsname::uname},
    unistd::mkdir,
};
use precisej_printable_errno::{ErrnoResult, PrintableErrno};
use std::path::Path;

/// Get current kernel version. Corresponds to the `release` field in the `utsname`
/// struct returned by `uname(2)`.
///
/// See `uname(2)` for more information.
pub fn get_booted_kernel_ver() -> String {
    uname().release().to_string()
}

/// Create `/run/initramfs`, which can be used by the booted system to switch back to
/// the initramfs environment on shutdown.
///
/// Per [systemd's INITRD_INTERFACE](https://systemd.io/INITRD_INTERFACE/).
pub fn make_shutdown_pivot_dir() -> Result<(), PrintableErrno<String>> {
    let s_rwxu_rxg_rxo =
        Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IXGRP | Mode::S_IROTH | Mode::S_IXOTH;
    loop {
        match mkdir(Path::new("/run/initramfs"), s_rwxu_rxg_rxo) {
            Ok(()) => break Ok(()),
            Err(e) if e == Errno::ENOENT => {
                // Recurse and try again
            }
            Err(e) => {
                break Err(e).printable(
                    PROGRAM_NAME,
                    "FATAL: unable to create /run/initramfs".to_string(),
                )
            }
        }
    }
}
