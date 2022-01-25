//! TODO

use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    fs::File,
    io::{Result as IoResult, Write},
    os::unix::fs::OpenOptionsExt,
};

#[repr(transparent)]
pub struct KmsgFmt(File);
impl KmsgFmt {
    pub fn new() -> Result<KmsgFmt, PrintableErrno<String>> {
        let file = File::options()
            .read(false)
            .write(true)
            .mode(nix::libc::S_IWUSR | nix::libc::S_IRUSR)
            .open("/dev/kmsg")
            .map_err(|io| {
                printable_error(PROGRAM_NAME, format!("unable to open /dev/kmsg: {}", io))
            })?;
        Ok(KmsgFmt(file))
    }

    pub(crate) fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.0.write_all(buf)
    }
}
