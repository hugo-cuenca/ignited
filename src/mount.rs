use crate::PROGRAM_NAME;
use nix::{
    errno::Errno,
    mount::{mount, MsFlags},
    sys::stat::Mode,
    unistd::mkdir,
};
use precisej_printable_errno::{printable_error, ErrnoResult, PrintableErrno};
use std::path::{Path, PathBuf};

pub struct TmpfsOpts {
    source: String,
    target: PathBuf,
    flags: MsFlags,
    options: Option<String>,
}
impl TmpfsOpts {
    pub fn new<S1: Into<String>, S2: Into<String>, P: Into<PathBuf>>(
        source: S1,
        target: P,
        flags: MsFlags,
        options: Option<S2>,
    ) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            flags,
            options: options.map(|s2| s2.into()),
        }
    }
}

#[warn(dead_code)]
pub enum Mount {
    DevTmpfs,
    DevPts,
    Proc,
    Sysfs,
    Tmpfs(TmpfsOpts),
    Efivarfs,
}
impl Mount {
    fn source(&self) -> &'_ str {
        match self {
            Mount::DevTmpfs => "dev",
            Mount::DevPts => "devpts",
            Mount::Proc => "proc",
            Mount::Sysfs => "sys",
            Mount::Tmpfs(TmpfsOpts { source, .. }) => source.as_str(),
            Mount::Efivarfs => "efivarfs",
        }
    }

    fn target(&self) -> &'_ Path {
        match self {
            Mount::DevTmpfs => Path::new("/dev"),
            Mount::DevPts => Path::new("/dev/pts"),
            Mount::Proc => Path::new("/proc"),
            Mount::Sysfs => Path::new("/sys"),
            Mount::Tmpfs(TmpfsOpts { target, .. }) => target.as_path(),
            Mount::Efivarfs => Path::new("/sys/firmware/efi/efivars"),
        }
    }

    fn fstype(&self) -> &'static str {
        match self {
            Mount::DevTmpfs => "devtmpfs",
            Mount::DevPts => "devpts",
            Mount::Proc => "proc",
            Mount::Sysfs => "sysfs",
            Mount::Tmpfs(_) => "tmpfs",
            Mount::Efivarfs => "efivarfs",
        }
    }

    fn flags(&self) -> MsFlags {
        match self {
            Mount::DevTmpfs => MsFlags::MS_NOSUID,
            Mount::DevPts => todo!(),
            Mount::Proc => MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            Mount::Sysfs => MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            Mount::Tmpfs(TmpfsOpts { flags, .. }) => *flags,
            Mount::Efivarfs => MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        }
    }

    fn options(&self) -> Option<&'_ str> {
        match self {
            Mount::DevTmpfs => Some("mode=0755"),
            Mount::DevPts => todo!(),
            Mount::Proc => None,
            Mount::Sysfs => None,
            Mount::Tmpfs(TmpfsOpts { ref options, .. }) => {
                match options {
                    Some(options) => Some(options.as_str()),
                    None => None,
                }
            },
            Mount::Efivarfs => None,
        }
    }

    fn mkdirall(path: &Path) -> Result<(), PrintableErrno<String>> {
        // Lifted from std's DirBuilder.create_dir_all()

        if path == Path::new("") {
            return Ok(());
        }

        let s_rwxu_rxg_rxo =
            Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IXGRP | Mode::S_IROTH | Mode::S_IXOTH;

        match mkdir(path, s_rwxu_rxg_rxo) {
            Ok(()) => return Ok(()),
            Err(e) if e == Errno::ENOENT => {
                // Recurse and try again
            }
            Err(_) if path.is_dir() => return Ok(()),
            Err(e) => {
                return Err(e).printable(
                    PROGRAM_NAME,
                    format!("FATAL: unable to create {}", path.to_string_lossy()),
                )
            }
        }
        match path.parent() {
            Some(p) => Self::mkdirall(p)?,
            None => {
                return Err(printable_error(
                    PROGRAM_NAME,
                    format!(
                        "FATAL: unable to create root tree for {}",
                        path.to_string_lossy()
                    ),
                ));
            }
        }
        match mkdir(path, s_rwxu_rxg_rxo) {
            Ok(()) => Ok(()),
            Err(_) if path.is_dir() => Ok(()),
            Err(e) => Err(e).printable(
                PROGRAM_NAME,
                format!(
                    "FATAL: unable to create root tree for {}",
                    path.to_string_lossy()
                ),
            ),
        }
    }

    pub fn mount(&self) -> Result<(), PrintableErrno<String>> {
        let target = self.target();
        Self::mkdirall(target)?;
        mount(
            Some(self.source()),
            self.target(),
            Some(self.fstype()),
            self.flags(),
            self.options(),
        )
        .printable(
            PROGRAM_NAME,
            format!("FATAL: unable to mount {}", target.to_string_lossy()),
        )?;
        Ok(())
    }
}
