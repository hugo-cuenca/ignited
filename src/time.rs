//! Keep track of time spent in the initramfs. Useful for systemd.

use crate::{early_logging::KConsole, PROGRAM_NAME};
use nix::time::{clock_gettime, ClockId};
use precisej_printable_errno::{printable_error, ErrnoResult, PrintableErrno};
use std::io::{Seek, SeekFrom, Write};

/// Keep track of time spent in the initramfs.
pub struct InitramfsTimer {
    realtime: Result<u64, PrintableErrno<String>>,
    monotonic: Result<u64, PrintableErrno<String>>,
}
impl InitramfsTimer {
    /// Start the timer.
    pub fn start() -> Self {
        let realtime = Self::read_clock(ClockId::CLOCK_REALTIME);
        let monotonic = Self::read_clock(ClockId::CLOCK_MONOTONIC);

        Self {
            realtime,
            monotonic,
        }
    }

    /// Log any errors associated with starting the timer.
    pub fn log(&self, kmsg: &mut KConsole) {
        if let Err(ref e) = self.realtime {
            kcrit!(kmsg, "{}", e);
        }
        if let Err(ref e) = self.monotonic {
            kcrit!(kmsg, "{}", e);
        }
    }

    /// Write the timer to a memfd, for usage with systemd-compatible `init`.
    pub fn write<W: Write + Seek>(self, dest: &mut W) -> Result<(), PrintableErrno<String>> {
        let realtime = self.realtime.unwrap_or_default();
        let monotonic = self.monotonic.unwrap_or_default();
        dest.write(format!("initrd-timestamp={} {}\n", realtime, monotonic).as_bytes())
            .map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("unable to write timer to destination for systemd: {}", io),
                )
            })?;
        dest.seek(SeekFrom::Start(0))
            .map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("unable to reset timer destination for systemd: {}", io),
                )
            })
            .map(|_| {})
    }

    // as booster's readClock(int32) in util.go
    fn read_clock(id: ClockId) -> Result<u64, PrintableErrno<String>> {
        let t =
            clock_gettime(id).printable(PROGRAM_NAME, format!("unable to read clock {}", id))?;
        let t_sec = t.tv_sec() as u64;
        let t_nsec = t.tv_nsec() as u64;
        Ok((t_sec * 1000000) + (t_nsec / 1000))
    }
}
