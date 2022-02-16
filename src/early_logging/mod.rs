//! Kernel logging infrastructure from userspace.
//!
//! The Linux kernel already contains a robust logging infrastructure internally
//! based on its `printk` buffer and extensible by userspace access to `/dev/kmsg`.
//! Various programs such as `rsyslog`, `syslog-ng`, and `systemd-journald` exist
//! that are able to, among other things, provide features on top of `/dev/kmsg`
//! such as remote access, disk persistence, corruption detection, among others.
//! This module contains macros and structures that allow easy logging of ignited's
//! messages to `/dev/kmsg` and reap the benefits of the comprehensive tooling
//! designed around it.

pub mod buf;
mod kmsg;

use crate::PROGRAM_NAME;
use kmsg::KmsgFmt;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    fs::{self, File},
    io::Write,
};

/// Userspace handle to the kernel buffer.
/// 
/// Once obtained through `Self::new()`, this handle is rarely used directly. Use the
/// various macros contained in this file instead, as they allow writing to the buffer
/// through this handle.
#[derive(Debug, Clone)]
pub struct KConsole {
    handle: KmsgFmt,
    current_level: VerbosityLevel,
}
impl KConsole {
    /// Attempt to open a new handle to the kernel buffer.
    ///
    /// Note: the default [VerbosityLevel] threshold (as defined in its implementation of
    /// [Default]) is used, and should be changed via [KConsole::change_verbosity] if requested.
    pub fn new() -> Result<KConsole, PrintableErrno<String>> {
        Ok(KConsole {
            handle: KmsgFmt::new()?,
            current_level: VerbosityLevel::default(),
        })
    }

    /// Write a new entry to the buffer.
    ///
    /// The entry will only be written if its [VerbosityLevel] is lower or equal to the threshold.
    #[inline]
    fn println(&mut self, req_level: VerbosityLevel, args: String) {
        if req_level <= self.current_level {
            self.handle
                .write(format!("<{}>{}: {}\n", req_level as u8, PROGRAM_NAME, args).as_bytes())
                .ok();
        }
    }

    /// Change the [VerbosityLevel] threshold.
    pub fn change_verbosity(&mut self, new_level: VerbosityLevel) {
        self.current_level = new_level;
    }

    /// Disable `kmsg` throttling when on [VerbosityLevel::Debug] threshold.
    ///
    /// Debug logging generates many messages. In order to preserve them all, we can disable
    /// `kmsg` throttling.
    pub fn disable_throttling_on_verbose(&self) -> Result<(), PrintableErrno<String>> {
        const SYS_KMSG_FILE: &str = "/proc/sys/kernel/printk_devkmsg";
        const NO_THROTTLE_ENABLED: &[u8] = b"on\n";

        if self.current_level != VerbosityLevel::Debug {
            return Ok(());
        }

        let data = fs::read(SYS_KMSG_FILE).map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("error while reading {}: {}", SYS_KMSG_FILE, io),
            )
        })?;
        if &data[..] == NO_THROTTLE_ENABLED {
            return Ok(());
        }

        File::options()
            .write(true)
            .open(SYS_KMSG_FILE)
            .map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while writing {}: {}", SYS_KMSG_FILE, io),
                )
            })?
            .write_all(NO_THROTTLE_ENABLED)
            .map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while writing {}: {}", SYS_KMSG_FILE, io),
                )
            })
    }
}

/// Log entry verbosity level, ranging from critical (`Crit`) to debug (`Debug`).
///
/// Default threshold is defined as `Info`.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum VerbosityLevel {
    /// `debug` verbosity level = 7.
    Debug = 7,

    /// `info` verbosity level = 6.
    Info = 6,

    /// `notice` verbosity level = 5.
    Notice = 5,

    /// `warn` or `warning` verbosity level = 4.
    Warn = 4,

    /// `err` or `error` verbosity level = 3.
    Err = 3,

    /// Critical verbosity level = 2. Not specifiable via command-line arguments.
    Crit = 2,
}
impl TryFrom<&str> for VerbosityLevel {
    type Error = ();

    fn try_from(level: &str) -> Result<Self, Self::Error> {
        match level {
            "debug" => Ok(VerbosityLevel::Debug),
            "info" => Ok(VerbosityLevel::Info),
            "notice" => Ok(VerbosityLevel::Notice),
            "warn" | "warning" => Ok(VerbosityLevel::Warn),
            "err" | "error" => Ok(VerbosityLevel::Err),
            _ => Err(()),
        }
    }
}
impl Default for VerbosityLevel {
    fn default() -> Self {
        VerbosityLevel::Info
    }
}

/// Write a new entry to the buffer.
///
/// Note: don't use this function directly. Use a convenience macro like `kinfo!()` instead.
#[doc(hidden)]
pub fn _print_message_ln(kcon: &mut KConsole, level: VerbosityLevel, args: String) {
    kcon.println(level, args)
}

/// Write a new entry to the buffer with [Debug verbosity][VerbosityLevel::Debug].
#[macro_export]
macro_rules! kdebug {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Debug, ::std::format!($($arg)*));
    })
}

/// Write a new entry to the buffer with [Info verbosity][VerbosityLevel::Info].
#[macro_export]
macro_rules! kinfo {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Info, ::std::format!($($arg)*));
    })
}

/// Write a new entry to the buffer with [Notice verbosity][VerbosityLevel::Notice].
#[macro_export]
macro_rules! knotice {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Notice, ::std::format!($($arg)*));
    })
}

/// Write a new entry to the buffer with [Warn verbosity][VerbosityLevel::Warn].
#[macro_export]
macro_rules! kwarn {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Warn, ::std::format!($($arg)*));
    })
}

/// Write a new entry to the buffer with [Err verbosity][VerbosityLevel::Err].
#[macro_export]
macro_rules! kerr {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Err, ::std::format!($($arg)*));
    })
}

/// Write a new entry to the buffer with [Crit verbosity][VerbosityLevel::Crit].
#[macro_export]
macro_rules! kcrit {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Crit, ::std::format!($($arg)*));
    })
}

/// For compile-time testing only. Should never be called.
#[doc(hidden)]
#[allow(dead_code)]
fn _test(kcon: &mut KConsole) {
    kdebug!(kcon, "TEST");
    kinfo!(kcon, "TEST");
    knotice!(kcon, "TEST");
    kwarn!(kcon, "TEST");
    kerr!(kcon, "TEST");
    kcrit!(kcon, "TEST");
}
