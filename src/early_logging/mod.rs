//! TODO

mod kmsg;

use crate::PROGRAM_NAME;
use kmsg::KmsgFmt;
use precisej_printable_errno::PrintableErrno;

pub struct KConsole {
    handle: KmsgFmt,
    current_level: VerbosityLevel,
}
impl KConsole {
    pub fn new() -> Result<KConsole, PrintableErrno<String>> {
        Ok(KConsole {
            handle: KmsgFmt::new()?,
            current_level: VerbosityLevel::default(),
        })
    }

    #[inline]
    fn println(&mut self, req_level: VerbosityLevel, args: String) {
        if req_level <= self.current_level {
            self.handle
                .write(format!("<{}>{}: {}\n", req_level as u8, PROGRAM_NAME, args).as_bytes())
                .ok();
        }
    }

    pub fn change_verbosity(&mut self, new_level: VerbosityLevel) {
        self.current_level = new_level;
    }
}

/// TODO
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum VerbosityLevel {
    Debug = 7,
    Info = 6,
    Notice = 5,
    Warn = 4,
    Err = 3,
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

#[doc(hidden)]
pub fn _print_message_ln(kcon: &mut KConsole, level: VerbosityLevel, args: String) {
    kcon.println(level, args)
}

#[macro_export]
macro_rules! kdebug {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Debug, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kinfo {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Info, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! knotice {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Notice, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kwarn {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Warn, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kerr {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Err, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kcrit {
    ($kcon:tt, $($arg:tt)*) => ({
        use ::std::borrow::BorrowMut;
        $crate::early_logging::_print_message_ln($kcon.borrow_mut(), $crate::early_logging::VerbosityLevel::Crit, ::std::format!($($arg)*));
    })
}

#[doc(hidden)]
fn _test(kcon: &mut KConsole) {
    kdebug!(kcon, "TEST");
    kinfo!(kcon, "TEST");
    knotice!(kcon, "TEST");
    kwarn!(kcon, "TEST");
    kerr!(kcon, "TEST");
    kcrit!(kcon, "TEST");
}
