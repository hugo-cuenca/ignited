use crate::{config::RuntimeConfig, early_logging::KConsole};
use precisej_printable_errno::PrintableErrno;

mod font {
    use crate::{early_logging::KConsole, PROGRAM_NAME};
    use precisej_printable_errno::{printable_error, PrintableErrno};
    use std::process::Command;

    /// Set the console font with `setfont`.
    pub fn set_font(
        kcon: &mut KConsole,
        font_file_path: Option<&str>,
        font_map_file_path: Option<&str>,
        font_unicode_file_path: Option<&str>,
    ) -> Result<(), PrintableErrno<String>> {
        if let Some(font_file_path) = font_file_path {
            kinfo!(kcon, "loading font file {}", font_file_path);

            let mut args = Vec::with_capacity(5);
            args.push(font_file_path.to_string());
            if let Some(font_map_file_path) = font_map_file_path {
                args.push("-m".to_string());
                args.push(font_map_file_path.to_string());
            }
            if let Some(font_unicode_file_path) = font_unicode_file_path {
                args.push("-u".to_string());
                args.push(font_unicode_file_path.to_string());
            }

            let command = Command::new("setfont").args(args).status().map_err(|io| {
                printable_error(PROGRAM_NAME, format!("unable to execute 'setfont': {}", io))
            })?;

            if !command.success() {
                return if let Some(code) = command.code() {
                    Err(printable_error(
                        PROGRAM_NAME,
                        format!(
                            "error while executing 'setfont': process exited with code {}",
                            code
                        ),
                    ))
                } else {
                    Err(printable_error(
                        PROGRAM_NAME,
                        "error while executing 'setfont': process signaled".to_string(),
                    ))
                };
            }
        }
        Ok(())
    }
}

mod keymap {
    use crate::{early_logging::KConsole, PROGRAM_NAME};
    use nix::{
        fcntl::{open, OFlag},
        ioctl_write_int_bad, ioctl_write_ptr_bad,
        sys::stat::Mode,
        unistd::write,
    };
    use precisej_printable_errno::{printable_error, ErrnoResult, PrintableErrno};
    use std::{fs, os::unix::io::RawFd};

    // from booster@anatol/init/console.go, originally from linux/kd.h
    const KDSKBMODE: i32 = 0x4B45;
    const KDSKBENT: i32 = 0x4B47;
    const K_XLATE: i32 = 0x01;
    const K_UNICODE: i32 = 0x03;
    const NR_KEYS: usize = 128;
    const MAX_NR_KEYMAPS: usize = 256;

    #[repr(C)]
    pub struct KbEntry {
        kb_table: u8,
        kb_index: u8,
        kb_value: u16,
    }

    ioctl_write_int_bad!(ioctl_kdskbmode, KDSKBMODE);
    ioctl_write_ptr_bad!(ioctl_kdskbent, KDSKBENT, KbEntry);

    fn load_keymap_file(vcon: RawFd, keymap_file_path: &str) -> Result<(), PrintableErrno<String>> {
        let keymap_blob = fs::read(keymap_file_path).map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("unable to open {}: {}", keymap_file_path, io),
            )
        })?;
        let keymap_blob = keymap_blob.strip_prefix(b"bkeymap").ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                format!(
                    "unable to process keymap file at {}: invalid keymap",
                    keymap_file_path
                ),
            )
        })?;
        let mut pos = MAX_NR_KEYMAPS;
        for (i, enabled_keymap) in (&keymap_blob[..MAX_NR_KEYMAPS]).iter().enumerate() {
            if *enabled_keymap != 1 {
                continue;
            }
            for j in 0..NR_KEYS {
                let ke = KbEntry {
                    kb_table: i as u8,
                    kb_index: j as u8,
                    kb_value: (&keymap_blob[pos] as *const _) as u16,
                };
                pos += 2;

                unsafe { ioctl_kdskbent(vcon, &ke) }
                    .printable(PROGRAM_NAME, "unable to change keymap")?;
            }
        }

        Ok(())
    }

    /// Update the kernel `tty0` keyboard mode and translation table with a keymap file.
    pub fn load_keymap(
        kcon: &mut KConsole,
        keymap_file_path: Option<&str>,
        is_utf8: bool,
    ) -> Result<(), PrintableErrno<String>> {
        if let Some(keymap_file_path) = keymap_file_path {
            kinfo!(kcon, "loading keymap file {}", keymap_file_path);
            let vcon = open("/dev/tty0", OFlag::O_RDWR, Mode::empty())
                .printable(PROGRAM_NAME, "unable to open tty0")?;

            let mode: i32;
            let ctrl: &'static [u8];
            if is_utf8 {
                mode = K_UNICODE;
                ctrl = b"\033%G";
            } else {
                mode = K_XLATE;
                ctrl = b"\033%@";
            }

            unsafe { ioctl_kdskbmode(vcon, mode) }
                .printable(PROGRAM_NAME, "unable to set keyboard mode")?;
            write(vcon, ctrl).printable(PROGRAM_NAME, "unable to set terminal line settings")?;

            load_keymap_file(vcon, keymap_file_path)?;
        }

        Ok(())
    }
}

/// Setup the virtual console.
///
/// Consists of setting the console's:
/// * desired font
/// * keyboard mode
/// * key translation tables
pub fn setup_vconsole(
    kcon: &mut KConsole,
    config: &RuntimeConfig,
) -> Result<(), PrintableErrno<String>> {
    if let Some(vconsole) = config.console() {
        font::set_font(
            kcon,
            vconsole.font_file(),
            vconsole.font_map_file(),
            vconsole.font_unicode_file(),
        )?;
        keymap::load_keymap(kcon, vconsole.keymap_file(), vconsole.is_utf8())?;
    }
    Ok(())
}
