use crate::{KConsole, PROGRAM_NAME};
use nix::{
    errno::Errno,
    mount::{mount, MsFlags},
    sys::stat::Mode,
    unistd::mkdir,
};
use precisej_printable_errno::{printable_error, ErrnoResult, PrintableErrno};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RootOpts {
    source: String,
    fstype: String,
    flags: MsFlags,
    options: Option<String>,
}
impl RootOpts {
    pub fn builder() -> RootOptsBuilder {
        Default::default()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct EfiPartitionGptGuid(uuid::Uuid);
impl EfiPartitionGptGuid {
    pub fn get_current() -> Result<Self, PrintableErrno<String>> {
        let (_, data) = Self::read_efi_var(
            "LoaderDevicePartUUID",
            "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f",
        )?;

        // FIXME: inefficient conversion: "u8"s -> utf-16 "u16"s -> utf-8 String -> "u8"s
        let uuid = {
            let mut utf16_uuid: Vec<u16> = Vec::with_capacity(data.len() / 2);
            for chunk in data.chunks(2) {
                let u16_int = match chunk.try_into() {
                    Ok(u16_b) => u16::from_le_bytes(u16_b),
                    Err(_) => chunk[0] as u16,
                };
                utf16_uuid.push(u16_int);
            }
            let uuid = String::from_utf16(&utf16_uuid[..]).map_err(|_| {
                printable_error(
                    PROGRAM_NAME,
                    "error while reading EFI variable: invalid UTF-16",
                )
            })?;
            uuid::Uuid::from_str(&uuid[..]).map_err(|_| {
                printable_error(
                    PROGRAM_NAME,
                    "error while reading EFI variable: invalid UUID",
                )
            })?
        };

        Ok(EfiPartitionGptGuid(uuid))
    }

    fn read_efi_var(name: &str, uuid: &str) -> Result<(u32, Vec<u8>), PrintableErrno<String>> {
        let data = std::fs::read(format!("/sys/firmware/efi/efivars/{}-{}", name, uuid)).map_err(
            |io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while reading EFI variable: {}", io),
                )
            },
        )?;

        let attr = u32::from_le_bytes((&data[..4]).try_into().map_err(|_| {
            printable_error(
                PROGRAM_NAME,
                "error while reading EFI variable: TryFromSliceError",
            )
        })?);

        Ok((attr, Vec::from(&data[4..])))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PartitionSourceBuilder {
    Uuid(uuid::Uuid),
    Label(String),
    PartUuid(uuid::Uuid),
    PartUuidPartnroff(uuid::Uuid, i64),
    PartType(uuid::Uuid, EfiPartitionGptGuid),
    PartLabel(String),
    RawDevice(String),
}
impl PartitionSourceBuilder {
    pub fn autodiscover_root(kcon: &mut KConsole) -> Result<Self, PrintableErrno<String>> {
        #[cfg(target_arch = "x86_64")]
        const ROOT_AUTODISC_UUID_TYPE: uuid::Uuid =
            compiled_uuid::uuid!("4f68bce3-e8cd-4db1-96e7-fbcaf984b709");

        #[cfg(target_arch = "x86")]
        const ROOT_AUTODISC_UUID_TYPE: uuid::Uuid =
            compiled_uuid::uuid!("44479540-f297-41b2-9af7-d131d5f0458a");

        #[cfg(target_arch = "arm")]
        const ROOT_AUTODISC_UUID_TYPE: uuid::Uuid =
            compiled_uuid::uuid!("69dad710-2ce4-4e3c-b16c-21a1d49abed3");

        #[cfg(target_arch = "aarch64")]
        const ROOT_AUTODISC_UUID_TYPE: uuid::Uuid =
            compiled_uuid::uuid!("b921b045-1df0-41c3-af44-4c6f280d3fae");

        kinfo!(
            kcon,
            "root= param is not specified. Using GPT partition autodiscovery with guid type {}",
            ROOT_AUTODISC_UUID_TYPE
        );
        Ok(Self::PartType(
            ROOT_AUTODISC_UUID_TYPE,
            EfiPartitionGptGuid::get_current()?,
        ))
    }

    #[inline]
    pub fn parse<R: AsRef<str>>(root: R) -> Option<Self> {
        Self::_parse(root.as_ref())
    }
    fn _parse(root: &str) -> Option<Self> {
        if let Some(uuid) = root
            .strip_prefix("UUID=")
            .or_else(|| root.strip_prefix("/dev/disk/by-uuid"))
        {
            Self::parse_uuid(uuid)
        } else if let Some(label) = root
            .strip_prefix("LABEL=")
            .or_else(|| root.strip_prefix("/dev/disk/by-label"))
        {
            Some(Self::parse_label(label))
        } else if let Some(partuuid_partnroff) = root.strip_prefix("PARTUUID=") {
            Self::parse_partuuid_partnroff(partuuid_partnroff)
        } else if let Some(partuuid) = root.strip_prefix("/dev/disk/by-partuuid") {
            Self::parse_partuuid(partuuid)
        } else if let Some(partlabel) = root
            .strip_prefix("PARTLABEL=")
            .or_else(|| root.strip_prefix("/dev/disk/by-partlabel"))
        {
            Some(Self::parse_partlabel(partlabel))
        } else if root.starts_with("/dev/") {
            let raw_device = root;
            Some(Self::parse_raw_device(raw_device))
        } else {
            None
        }
    }

    fn parse_label(label: &str) -> Self {
        Self::Label(label.to_string())
    }

    fn parse_partlabel(partlabel: &str) -> Self {
        Self::PartLabel(partlabel.to_string())
    }

    fn parse_partuuid(partuuid: &str) -> Option<Self> {
        Some(Self::PartUuid(Self::uuid_from_str(partuuid)?))
    }

    fn parse_partuuid_partnroff(partuuid_partnroff: &str) -> Option<Self> {
        if let Some((partuuid, partnroff)) = partuuid_partnroff.split_once("/PARTNROFF=") {
            let partnroff = i64::from_str(partnroff).ok()?;
            let partuuid = Self::uuid_from_str(partuuid)?;
            Some(Self::PartUuidPartnroff(partuuid, partnroff))
        } else {
            let partuuid = partuuid_partnroff;
            Self::parse_partuuid(partuuid)
        }
    }

    fn parse_raw_device(raw_device: &str) -> Self {
        Self::RawDevice(raw_device.to_string())
    }

    fn parse_uuid(uuid: &str) -> Option<Self> {
        Some(Self::Uuid(Self::uuid_from_str(uuid)?))
    }

    // uuid is possible quoted, should be stripped before processing
    fn uuid_from_str(uuid_str_q: &str) -> Option<uuid::Uuid> {
        let uuid_str = uuid_str_q
            .strip_prefix('"')
            .map(|u| u.strip_suffix('"'))
            .flatten()
            .unwrap_or(uuid_str_q);
        uuid::Uuid::from_str(uuid_str).ok()
    }

    pub fn build(self) -> String {
        todo!("convert to device path")
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RootOptsBuilder {
    source: Option<PartitionSourceBuilder>,
    fstype: Option<String>,
    rw: bool,
    flags: MsFlags,
    options: Option<String>,
}
impl RootOptsBuilder {
    pub fn source(&mut self, source: PartitionSourceBuilder) -> &mut Self {
        self.source.get_or_insert(source);
        self
    }

    pub fn get_source(&self) -> Option<&PartitionSourceBuilder> {
        self.source.as_ref()
    }

    #[inline]
    pub fn fstype<F: Into<String>>(&mut self, fstype: F) -> &mut Self {
        self._fstype(fstype.into());
        self
    }
    fn _fstype(&mut self, fstype: String) {
        self.fstype.get_or_insert(fstype);
    }

    pub fn get_fstype(&self) -> Option<&str> {
        self.fstype.as_deref()
    }

    pub fn ro(&mut self) -> &mut Self {
        self.rw = false;
        self
    }

    pub fn rw(&mut self) -> &mut Self {
        self.rw = true;
        self
    }

    #[inline]
    pub fn add_opts<O: AsRef<str>>(&mut self, o: O) -> &mut Self {
        self._add_opts(o.as_ref());
        self
    }
    fn _add_opts(&mut self, opts: &str) {
        for opt in opts.split(',') {
            match opt {
                "dirsync" => self.flags.insert(MsFlags::MS_DIRSYNC),
                "nolazytime" => self.flags.remove(MsFlags::MS_LAZYTIME),
                "lazytime" => self.flags.insert(MsFlags::MS_LAZYTIME),
                "noatime" => self.flags.insert(MsFlags::MS_NOATIME),
                "atime" => self.flags.remove(MsFlags::MS_NOATIME),
                "nodev" => self.flags.insert(MsFlags::MS_NODEV),
                "dev" => self.flags.remove(MsFlags::MS_NODEV),
                "nodiratime" => self.flags.insert(MsFlags::MS_NODIRATIME),
                "diratime" => self.flags.remove(MsFlags::MS_NODIRATIME),
                "noexec" => self.flags.insert(MsFlags::MS_NOEXEC),
                "exec" => self.flags.remove(MsFlags::MS_NOEXEC),
                "nosuid" => self.flags.insert(MsFlags::MS_NOSUID),
                "suid" => self.flags.remove(MsFlags::MS_NOSUID),
                "norelatime" => self.flags.remove(MsFlags::MS_RELATIME),
                "relatime" => self.flags.insert(MsFlags::MS_RELATIME),
                "silent" => self.flags.insert(MsFlags::MS_SILENT),
                "nostrictatime" => self.flags.remove(MsFlags::MS_STRICTATIME),
                "strictatime" => self.flags.insert(MsFlags::MS_STRICTATIME),
                "async" => self.flags.remove(MsFlags::MS_SYNCHRONOUS),
                "sync" => self.flags.insert(MsFlags::MS_SYNCHRONOUS),
                "nosymfollow" => {
                    // FIXME: suggest adding MsFlags::MS_NOSYMFOLLOW to nix
                    // TODO: document lack of options
                }
                option => {
                    match self.options {
                        Some(ref mut options) => {
                            options.push(',');
                            options.push_str(option);
                        }
                        None => self.options = Some(option.to_string()),
                    };
                }
            }
        }
    }

    pub fn try_build(self) -> Result<RootOpts, Self> {
        let (source, fstype) = match (&self.source, &self.fstype) {
            (Some(source), Some(fstype)) => (source.clone().build(), fstype.clone()),
            _ => return Err(self),
        };
        let options = self.options;

        let mut flags = self.flags;
        flags.set(MsFlags::MS_RDONLY, !self.rw);

        Ok(RootOpts {
            source,
            fstype,
            flags,
            options,
        })
    }

    // TODO document panic on unwrap/incomplete
    pub fn build(self) -> RootOpts {
        let source = self.source.unwrap().build();
        let fstype = self.fstype.unwrap();

        let options = self.options;

        let mut flags = self.flags;
        flags.set(MsFlags::MS_RDONLY, !self.rw);

        RootOpts {
            source,
            fstype,
            flags,
            options,
        }
    }
}
impl Default for RootOptsBuilder {
    fn default() -> Self {
        RootOptsBuilder {
            source: None,
            fstype: None,
            rw: false,
            flags: MsFlags::empty(),
            options: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Mount {
    DevTmpfs,
    DevPts,
    Proc,
    Sysfs,
    Tmpfs(TmpfsOpts),
    Efivarfs,
    Root(RootOpts),
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
            Mount::Root(RootOpts { source, .. }) => source.as_str(),
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
            Mount::Root(_) => todo!(),
        }
    }

    fn fstype(&self) -> &'_ str {
        match self {
            Mount::DevTmpfs => "devtmpfs",
            Mount::DevPts => "devpts",
            Mount::Proc => "proc",
            Mount::Sysfs => "sysfs",
            Mount::Tmpfs(_) => "tmpfs",
            Mount::Efivarfs => "efivarfs",
            Mount::Root(RootOpts { fstype, .. }) => fstype.as_str(),
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
            Mount::Root(RootOpts { flags, .. }) => *flags,
        }
    }

    fn options(&self) -> Option<&'_ str> {
        match self {
            Mount::DevTmpfs => Some("mode=0755"),
            Mount::DevPts => todo!(),
            Mount::Proc => None,
            Mount::Sysfs => None,
            Mount::Tmpfs(TmpfsOpts { ref options, .. }) => options.as_deref(),
            Mount::Efivarfs => None,
            Mount::Root(RootOpts { ref options, .. }) => options.as_deref(),
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
