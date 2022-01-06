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
#![deny(unused)]
#![deny(unstable_features)]
#![warn(missing_docs)]
#![allow(rustdoc::private_intra_doc_links)]

use cstr::cstr;
use std::ffi::CStr;
use precisej_printable_errno::{ExitError, PrintableErrno, PrintableResult};

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
const INIT_PATH: &'static CStr = cstr!("/sbin/serviced");

/// Error message used in case `INIT_PATH` is not able to be executed by `execv`.
/// This can be caused by not having init installed in the right path with the
/// proper executable permissions.
const INIT_ERROR: &'static str = "unable to execute init";

/// Check if inside initrd
fn initial_sanity_check() -> Result<(), PrintableErrno<&'static str>> {
    Ok(())
}

/// The entry point of the program. This function is in charge of exiting with an error
/// code when [init] returns an [ExitError].
fn main() {
    if let Err(e) = init() {
        e.eprint_and_exit()
    }
}

/// Here is where it actually begins.
///
/// TODO write docs
fn init() -> Result<(), ExitError<String>> {
    initial_sanity_check().bail(1)?;

    // perform ignition

    Ok(())
}