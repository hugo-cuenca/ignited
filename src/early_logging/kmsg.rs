use crate::PROGRAM_NAME;
use nix::{
    fcntl::{open, OFlag},
    sys::stat::Mode,
};
use precisej_printable_errno::{ErrnoResult, PrintableErrno};
use std::{
    fs::File,
    io::{Result as IoResult, Write},
    os::unix::prelude::FromRawFd,
};

#[repr(transparent)]
pub struct KmsgFmt(File);
impl KmsgFmt {
    pub fn new() -> Result<KmsgFmt, PrintableErrno<&'static str>> {
        let kmsg_fd = open("/dev/kmsg", OFlag::O_WRONLY, Mode::S_IWUSR | Mode::S_IRUSR)
            .printable(PROGRAM_NAME, "unable to open /dev/kmsg")?;
        Ok(KmsgFmt(unsafe { File::from_raw_fd(kmsg_fd) }))
    }

    pub(crate) fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.0.write_all(buf)
    }
}
