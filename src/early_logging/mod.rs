//! TODO

mod kmsg;

use crate::PROGRAM_NAME;
use kmsg::KmsgFmt;
use precisej_printable_errno::PrintableErrno;

#[repr(transparent)]
pub struct KConsole(KmsgFmt);

impl KConsole {
    pub fn new() -> Result<KConsole, PrintableErrno<&'static str>> {
        Ok(KConsole(KmsgFmt::new()?))
    }
}

#[doc(hidden)]
pub fn _print_message_ln(kcon: &mut KConsole, level: u8, args: String) {
    kcon.0
        .write(format!("<{}>{}: {}\n", level, PROGRAM_NAME, args).as_bytes())
        .ok();
}

#[macro_export]
macro_rules! kdebug {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 7, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kinfo {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 6, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! knotice {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 5, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kwarning {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 4, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kerr {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 3, ::std::format!($($arg)*));
    })
}

#[macro_export]
macro_rules! kcrit {
    ($kcon:tt, $($arg:tt)*) => ({
        $crate::early_logging::_print_message_ln(&mut $kcon, 2, ::std::format!($($arg)*));
    })
}
