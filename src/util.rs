//! Miscellaneous functions that don't fit in any other (rust code) module.

use crate::{early_logging::KConsole, PROGRAM_NAME};
use cstr::cstr;
use nix::{
    errno::Errno,
    sys::{stat::Mode, utsname::uname},
    unistd::{execv, mkdir},
};
use precisej_printable_errno::{ErrnoResult, PrintableErrno};
use std::{convert::Infallible, path::Path};

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

/// Spawn an emergency shell.
///
/// Currently this function attempts to spawn `/bin/busybox` first. If it doesn't exist,
/// it will attempt `/bin/toybox` instead. If none exists (or all of them fail to
/// properly handover execution), this function logs an error to `kmsg`.
///
/// If the emergency shell is spawned, this function never returns.
pub fn spawn_emergency_shell(kcon: &mut KConsole) -> Result<Infallible, ()> {
    kcrit!(kcon, "attempting to spawn emergency shell");

    let argv = [cstr!("sh"), cstr!("-I")];
    let exists_b = match execv(cstr!("/bin/busybox"), &argv).unwrap_err() {
        Errno::ENOENT => false,
        e => {
            let e = Err::<(), _>(e)
                .printable(PROGRAM_NAME, "unable to execute /bin/busybox")
                .unwrap_err();
            kcrit!(kcon, "{}", e);
            true
        }
    };

    // If we are here, busybox doesn't exist or execv failed, so try toybox
    let err_t = match execv(cstr!("/bin/toybox"), &argv).unwrap_err() {
        Errno::ENOENT => None,
        e => Some(
            Err::<(), _>(e)
                .printable(PROGRAM_NAME, "unable to execute /bin/toybox")
                .unwrap_err(),
        ),
    };
    // Both failed to execute
    if !exists_b {
        kcrit!(
            kcon,
            "unable to execute /bin/busybox: {}",
            Errno::ENOENT.desc()
        );
    }

    match err_t {
        Some(e) => {
            kcrit!(kcon, "{}", e);
        }
        None => {
            kcrit!(
                kcon,
                "unable to execute /bin/toybox: {}",
                Errno::ENOENT.desc()
            );
        }
    }

    Err(())
}
