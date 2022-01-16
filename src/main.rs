//! Rapid early-ramdisk system initialization, accompanying initramfs generator. **CURRENTLY IN DEVELOPMENT**
//!
//! # What?
//! ignited is a simple program meant to run before your Linux system partition has even been
//! mounted! It is intended to run inside your initramfs and bring your system to a usable
//! state, at which point it hands off execution to your `/sbin/init` program.
//!
//! ignited, although written in Rust, is based on [Booster](https://github.com/anatol/booster)
//! which is written in Go. Booster is licensed under MIT, a copy of which can be found
//! [here](https://raw.githubusercontent.com/anatol/booster/master/LICENSE).
//!
//! ## What is an initramfs?
//! From Booster's README:
//!
//!     Initramfs is a specially crafted small root filesystem
//!     that mounted at the early stages of Linux OS boot process.
//!     This initramfs among other things is responsible for
//!     [...] mounting [the] root filesystem.
//!
//! In other words, the initramfs is in charge of bringing the system to a usable state,
//! mounting the root filesystem, and handing off further work to your system's init.
//!
//! # Where?
//! ignited only supports Linux systems, and there are no plans to expand compatibility to
//! other OSes.
//!
//! If any Linux distribution maintainer/developer is exploring the use of ignited inside
//! their initramfs archives (or is already doing so), please contact me to be included in
//! this README. I am also able to give recommendations on proper integration in a distribution
//! if you contact me.
//!
//! # Why?
//! ## Why not booster?
//! ## Why not mkinitcpio?
//! ## Why not dracut?
//! # How?
//! # initd?
//! To be written.
//!
//! ***
//!
//! This crate currently serves the purpose of reserving the name `ignited` in crates.io,
//! and contains no other code than the standard "Hello, world!".
#![crate_name = "ignited"]
#![cfg_attr(test, deny(warnings))]
// #![deny(unused)] // TODO
#![deny(unstable_features)]
#![warn(missing_docs)]
#![allow(rustdoc::private_intra_doc_links)]

#[macro_use]
mod early_logging;

mod config;
mod mount;
mod util;

use crate::{
    config::{InitramfsMetadata, RuntimeConfig},
    early_logging::KConsole,
    mount::{Mount, TmpfsOpts},
    util::get_booted_kernel_ver,
};
use cstr::cstr;
use nix::mount::MsFlags;
use precisej_printable_errno::{
    printable_error, ExitError, ExitErrorResult, PrintableErrno, PrintableResult,
};
use std::{
    ffi::{CStr, OsStr},
    path::Path,
    process::id as getpid,
};

/// The program is called `ignited`. The str referring to the program name is saved in
/// this constant. Useful for PrintableResult.
const PROGRAM_NAME: &'static str = "ignited";

/// Path where init is located. Used in the `execv` call to actually execute
/// init.
///
/// **Note**: if you are a distribution maintainer, make sure your serviced package
/// actually puts the executable in `/sbin/init`. Otherwise, you must maintain a
/// patch changing `INIT_PATH` to the appropriate path (e.g. `/init`,
/// `/bin/init`, or `/usr/bin/init`).
const INIT_PATH: &'static CStr = cstr!("/sbin/init");

/// Error message used in case `INIT_PATH` is not able to be executed by `execv`.
/// This can be caused by not having init installed in the right path with the
/// proper executable permissions.
const INIT_ERROR: &'static str = "unable to execute init";

/// Path where `ignited`'s config file is located. TODO
const INIT_CONFIG: &'static str = "/etc/ignited/engine.toml";

/// Check if inside initrd. TODO
fn initial_sanity_check() -> Result<(), PrintableErrno<String>> {
    // We must be the initramfs' PID1
    (getpid() == 1).then(|| ()).ok_or_else(|| {
        printable_error(
            PROGRAM_NAME,
            "not running in an initrd environment, exiting...",
        )
    })?;

    // Per https://systemd.io/INITRD_INTERFACE/, we should only run if /etc/initrd-release
    // is present
    Path::new("/etc/initrd-release")
        .exists()
        .then(|| ())
        .ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                "not running in an initrd environment, exiting...",
            )
        })?;

    Ok(())
}

/// Perform initial work. TODO
fn initialize_kcon() -> Result<KConsole, PrintableErrno<String>> {
    Mount::DevTmpfs.mount()?;

    // /dev should be mounted at this point
    let mut kcon = KConsole::new()?;
    kdebug!(kcon, "mounted /dev");
    kdebug!(kcon, "hooked up to kmsg!");
    Ok(kcon)
}

/// Check if booted kernel version matches initrd kernel version. TODO
fn kernel_ver_check(config: InitramfsMetadata) -> Result<(), PrintableErrno<String>> {
    let cur_ver = &get_booted_kernel_ver()[..];
    let conf_ver = config.kernel_ver();
    (cur_ver == conf_ver)
        .then(|| ())
        .ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                format!(
                    "Linux kernel version mismatch. This initramfs image was built for version {con} and it is incompatible with the currently running version {cur}. Please rebuild the ignited image for kernel {cur}.",
                    con = conf_ver,
                    cur = cur_ver,
                ),
            )
        })
}

/// The entry point of the program. This function is in charge of exiting with an error
/// code when [init] returns an [ExitError].
fn main() {
    initial_sanity_check().bail(1).unwrap_or_eprint_exit();
    let mut kcon = initialize_kcon().bail(2).unwrap_or_eprint_exit();

    if let Err(e) = init(&mut kcon) {
        kcrit!(kcon, "{}", &e);
        e.eprint_and_exit()
    }
}

/// Here is where it actually begins.
///
/// TODO write docs
fn init(kcon: &mut KConsole) -> Result<(), ExitError<String>> {
    // Commence ignition
    kinfo!(kcon, "performing ignition...");
    Mount::Sysfs.mount().bail(3)?;
    kdebug!(kcon, "mounted /sys");
    Mount::Proc.mount().bail(3)?;
    kdebug!(kcon, "mounted /proc");
    Mount::Tmpfs(TmpfsOpts::new(
        "run",
        Path::new("/run"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_STRICTATIME,
        Some("mode=755"),
    ))
    .mount()
    .bail(3)?;
    kdebug!(kcon, "mounted /tmp");

    // If we are booted in EFI mode, we should mount efivarfs
    if Path::new("/sys/firmware/efi").exists() {
        kdebug!(kcon, "booted in efi mode");
        Mount::Efivarfs.mount().bail(3)?;
        kdebug!(kcon, "mounted /sys/firmware/efi/efivars");
    } else {
        kdebug!(kcon, "booted in bios/legacy mode");
    }

    std::env::set_var("PATH", OsStr::new("/usr/bin")); // Panics on error

    let config = RuntimeConfig::try_from(Path::new(INIT_CONFIG)).bail(4)?;

    kernel_ver_check(config.metadata()).bail(5)?;
    kdebug!(
        kcon,
        "passed kernel version match, can proceed to loading modules when ready"
    );

    Ok(())
}
