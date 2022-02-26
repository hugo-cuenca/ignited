//! `/dev/kmsg`-specific code.

use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    fs::File,
    io::{Result as IoResult, Write},
    os::unix::fs::OpenOptionsExt,
};

/// Contains the file descriptor corresponding to `/dev/kmsg`.
#[derive(Debug)]
#[repr(transparent)]
pub struct KmsgFmt(File);
impl KmsgFmt {
    /// Open `/dev/kmsg`.
    pub fn new() -> Result<Self, PrintableErrno<String>> {
        let file = File::options()
            .read(false)
            .write(true)
            .mode(nix::libc::S_IWUSR | nix::libc::S_IRUSR)
            .open("/dev/kmsg")
            .map_err(|io| {
                printable_error(PROGRAM_NAME, format!("unable to open /dev/kmsg: {}", io))
            })?;
        Ok(Self(file))
    }

    /// Write to `/dev/kmsg`.
    pub(crate) fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.0.write_all(buf)
    }
}
impl Clone for KmsgFmt {
    fn clone(&self) -> Self {
        // opening /dev/kmsg shouldn't fail (especially if it already succeeded once),
        // so if it does assume its a spurious error and try again.
        loop {
            match KmsgFmt::new() {
                Ok(res) => break res,
                Err(_) => {
                    // Sleep and try again.
                    // TODO figure out how to kerr!() since kmsg is already open
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }
}
