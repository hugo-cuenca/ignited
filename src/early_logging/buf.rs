use crate::{
    early_logging::{KConsole, VerbosityLevel, _print_message_ln},
    kcrit, kdebug, kerr, kinfo, knotice, kwarn,
};

struct KmsgBufEntry {
    level: VerbosityLevel,
    args: String,
}
pub struct KmsgBuf<'a> {
    inner_con: &'a mut KConsole,
    inner_buf: Vec<KmsgBufEntry>,
    flushed: bool,
}
impl<'a> KmsgBuf<'a> {
    pub fn new(kcon: &'a mut KConsole) -> Self {
        Self {
            inner_con: kcon,
            inner_buf: Default::default(),
            flushed: false,
        }
    }

    #[inline]
    pub fn kdebug(&mut self, args: String) {
        self._kany(VerbosityLevel::Debug, args)
    }

    #[inline]
    pub fn kwarn(&mut self, args: String) {
        self._kany(VerbosityLevel::Warn, args)
    }

    pub fn flush_with_level(&mut self, level: VerbosityLevel) {
        let buf = &mut self.inner_buf;

        self.inner_con.change_verbosity(level);
        self.flushed = true;
        for entry in buf.drain(..buf.len()) {
            _print_message_ln(self.inner_con, entry.level, entry.args)
        }
    }

    fn _kany(&mut self, level: VerbosityLevel, args: String) {
        if self.flushed {
            Self::_println(self.inner_con, level, args)
        } else {
            self.inner_buf.push(KmsgBufEntry { level, args })
        }
    }

    fn _println(kcon: &mut KConsole, level: VerbosityLevel, args: String) {
        match level {
            VerbosityLevel::Debug => kdebug!(kcon, "{}", args),
            VerbosityLevel::Info => kinfo!(kcon, "{}", args),
            VerbosityLevel::Notice => knotice!(kcon, "{}", args),
            VerbosityLevel::Warn => kwarn!(kcon, "{}", args),
            VerbosityLevel::Err => kerr!(kcon, "{}", args),
            VerbosityLevel::Crit => kcrit!(kcon, "{}", args),
        }
    }
}
