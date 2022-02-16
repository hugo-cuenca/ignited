//! Ignited configuration through command-line arguments and `/etc/ignited/engine.toml`.

use crate::{
    early_logging::{buf::KmsgBuf, KConsole, VerbosityLevel},
    module::ModParams,
    mount::{PartitionSourceBuilder, RootOpts, RootOptsBuilder},
    INIT_DEFAULT_PATH, PROGRAM_NAME,
};
use precisej_printable_errno::{printable_error, PrintableErrno, PrintableResult};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::{read_to_string, File},
    io::Read,
    path::Path,
};

// Inner struct for InitramfsMetadata deserialization
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
struct InitramfsMetadataDe {
    #[serde(rename = "kver")]
    kernel_ver: String,

    module_builtin: Vec<String>,
    module_deps: BTreeMap<String, Vec<String>>,
    module_opts: BTreeMap<String, String>,
    module_post_deps: BTreeMap<String, Vec<String>>,
}

/// \[metadata] section.
///
/// Example:
///
/// ```toml
/// [metadata]
/// kver = "5.10.95-hardened1-1-hardened"
/// module-builtin = ["foobar", "baz"]
///
/// [metadata.module-deps]
/// foo = ["bar", "mane"]
/// padme = ["hum"]
/// #[...]
///
/// [metadata.module-opts]
/// fee = "fie,foe"
/// #[...]
///
/// [metadata.module-post-deps]
/// fb = ["fizz", "buzz", "fizzbuzz"]
/// #[...]
/// ```
///
/// See the documentation in each function for more details on how this section is structured
#[repr(transparent)]
pub struct InitramfsMetadata<'a>(&'a InitramfsMetadataDe);
impl<'a> InitramfsMetadata<'a> {
    /// (String) The kernel version this initramfs was built for.
    ///
    /// ```toml
    /// [metadata]
    /// kver = "5.15.16-hardened1-1-precise"
    /// ```
    pub fn kernel_ver(&'_ self) -> &'_ str {
        &self.0.kernel_ver[..]
    }

    /// (Array\[String]) Modules that are already built-in to the kernel.
    ///
    /// ```toml
    /// [metadata]
    /// module-builtin = ["lkrg", "tirdad"]
    /// ```
    pub fn module_builtin(&'_ self) -> &'_ [String] {
        &self.0.module_builtin[..]
    }

    /// (Table: String > Array\[String]) Module (pre-)dependencies.
    ///
    /// ```toml
    /// [metadata.module-deps]
    /// precise_sec = ["lkrg", "tirdad"]
    /// ```
    pub fn module_deps(&'_ self) -> &'_ BTreeMap<String, Vec<String>> {
        &self.0.module_deps
    }

    /// (Table: String > String) Module options.
    ///
    /// ```toml
    /// [metadata.module-opts]
    /// sqlitefs = "checkpoint,fscrypt"
    /// ```
    pub fn module_opts(&'_ self) -> &'_ BTreeMap<String, String> {
        &self.0.module_opts
    }

    /// (Table: String > Array\[String]) Module post-dependencies.
    ///
    /// ```toml
    /// [metadata.module-post-deps]
    /// custom_lsm = ["custom_lsm_enforce", "custom_lsm_ioctl"]
    /// ```
    pub fn module_post_deps(&'_ self) -> &'_ BTreeMap<String, Vec<String>> {
        &self.0.module_post_deps
    }
}

// Inner struct for IgnitedConfig deserialization
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
struct IgnitedConfigDe {
    lvm: bool,
    mdraid: bool,
    module_force: Vec<String>,
    mount_timeout: Option<i64>,
}

/// \[ignited] section.
///
/// Example:
///
/// ```toml
/// [ignited]
/// lvm = false
/// mdraid = false
/// module-force = ["foo", "bar", "baz", "foobar"]
/// mount-timeout = 120
/// ```
///
/// See the documentation in each function for more details on how this section is structured
#[repr(transparent)]
pub struct IgnitedConfig<'a>(&'a IgnitedConfigDe);
impl<'a> IgnitedConfig<'a> {
    /// (Boolean) Whether LVM is required to mount the root partition.
    ///
    /// ```toml
    /// [ignited]
    /// lvm = true
    /// ```
    pub fn has_lvm(&self) -> bool {
        self.0.lvm
    }

    /// (Boolean) Whether RAID is required to mount the root partition.
    ///
    /// ```toml
    /// [ignited]
    /// mdraid = true
    /// ```
    pub fn has_mdraid(&self) -> bool {
        self.0.mdraid
    }

    /// (Array\[String]) Force specific kernel modules to load during initramfs.
    ///
    /// ```toml
    /// [ignited]
    /// module-force = ["bus1", "ashmem", "sqlitefs"]
    /// ```
    pub fn get_force_modules(&'_ self) -> &'_ [String] {
        &self.0.module_force[..]
    }

    /// (Optional: Integer) Root mount timeout in seconds. If no timeout is desired, omit
    /// the key or set the value to less than or equal to zero.
    ///
    /// ```toml
    /// [ignited]
    /// mount-timeout = 120
    /// ```
    pub fn get_mount_timeout(&self) -> Option<i64> {
        self.0.mount_timeout.filter(|m| *m > 0)
    }
}

// Inner struct for ConsoleConfig deserialization
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
struct ConsoleConfigDe {
    utf: bool,

    #[serde(rename = "font-file")]
    font_file_p: String,

    #[serde(rename = "font-map-file")]
    font_map_file_p: String,

    #[serde(rename = "font-unicode-file")]
    font_unicode_file_p: String,

    #[serde(rename = "keymap-file")]
    keymap_file_p: String,
}

/// \[console] section.
///
/// Example:
///
/// ```toml
/// [console]
/// utf = true
/// font-file = "/path/to/foo"
/// font-map-file = "/path/to/bar"
/// font-unicode-file = "/path/to/baz"
/// keymap-file = "/path/to/foobar"
/// ```
///
/// See the documentation in each function for more details on how this section is structured
#[repr(transparent)]
pub struct ConsoleConfig<'a>(&'a ConsoleConfigDe);
impl<'a> ConsoleConfig<'a> {
    /// (Boolean) Whether UTF-8 is to be used in the console.
    ///
    /// ```toml
    /// [console]
    /// utf8 = false
    /// ```
    pub fn is_utf8(&self) -> bool {
        self.0.utf
    }

    /// (String) Path to console font
    ///
    /// ```toml
    /// [console]
    /// font-file = "/usr/share/kbd/consolefonts/LatArCyrHeb-19.psfu.gz"
    /// ```
    pub fn font_file(&'_ self) -> &'_ str {
        &self.0.font_file_p[..]
    }

    /// (String) Path to console font map
    ///
    /// ```toml
    /// [console]
    /// font-map-file = "/usr/share/kbd/consoletrans/viscii1.0_to_tcvn.trans"
    /// ```
    pub fn font_map_file(&'_ self) -> &'_ str {
        &self.0.font_map_file_p[..]
    }

    /// (String) Path to console font unicode map
    ///
    /// ```toml
    /// [console]
    /// font-unicode-file = "/usr/share/kbd/unimaps/iso01.uni"
    /// ```
    pub fn font_unicode_file(&'_ self) -> &'_ str {
        &self.0.font_unicode_file_p[..]
    }

    /// (String) Path to console keymap
    ///
    /// ```toml
    /// [console]
    /// font-unicode-file = "/usr/share/kbd/keymaps/i386/dvorak/dvorak-programmer.map.gz"
    /// ```
    pub fn keymap_file(&'_ self) -> &'_ str {
        &self.0.keymap_file_p[..]
    }
}

/// Ignited TOML configuration file.
///
/// `/etc/ignited/engine.toml` should be TOML file with three main sections:
///
/// [`[ignited]`][IgnitedConfig]
///
/// [`[metadata]`][InitramfsMetadata]
///
/// [`[console]`][ConsoleConfig] # (Optional)
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct RuntimeConfig {
    metadata: InitramfsMetadataDe,
    ignited: IgnitedConfigDe,
    console: Option<ConsoleConfigDe>,
}
impl RuntimeConfig {
    /// `[metadata]`
    pub fn metadata(&self) -> InitramfsMetadata<'_> {
        InitramfsMetadata(&self.metadata)
    }

    /// `[ignited]`
    pub fn sysconf(&self) -> IgnitedConfig<'_> {
        IgnitedConfig(&self.ignited)
    }

    /// `[console]`
    pub fn console(&self) -> Option<ConsoleConfig<'_>> {
        self.console.as_ref().map(ConsoleConfig)
    }
}
impl TryFrom<&str> for RuntimeConfig {
    type Error = PrintableErrno<String>;

    #[inline(always)]
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        toml::from_str(value).map_err(|de| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", de))
        })
    }
}
impl TryFrom<String> for RuntimeConfig {
    type Error = PrintableErrno<String>;

    #[inline(always)]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(&value[..])
    }
}
impl TryFrom<File> for RuntimeConfig {
    type Error = PrintableErrno<String>;

    #[inline(always)]
    fn try_from(mut value: File) -> Result<Self, Self::Error> {
        let mut out = String::with_capacity(1024);
        value.read_to_string(&mut out).map_err(|io| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
        })?;
        Self::try_from(out)
    }
}
impl TryFrom<&std::path::Path> for RuntimeConfig {
    type Error = PrintableErrno<String>;

    #[inline(always)]
    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let out = read_to_string(value).map_err(|io| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
        })?;
        Self::try_from(out)
    }
}

/// Boot-time custom arguments.
///
/// Whichever boot loader you choose to use (GRUB, systemd-boot, Limine, etc.) should have
/// support for boot-time custom arguments (called `GRUB_CMDLINE_LINUX` in GRUB, `options` in
/// [The Boot Loader Specification](https://systemd.io/BOOT_LOADER_SPECIFICATION/#type-1-boot-loader-specification-entries),
/// `CMDLINE` in Limine, etc). This struct allows for the parsing of arguments relevant in
/// userspace initialization through `/proc/cmdline`.
#[derive(Debug, Clone)]
pub struct CmdlineArgs {
    init: CString,
    root_opts: RootOptsBuilder,
    resume_source: Option<PartitionSourceBuilder>,
    mod_params: ModParams,
}
impl CmdlineArgs {
    /// Parse the current boot-time arguments in `/proc/cmdline`.
    pub fn parse_current(kcon: &mut KConsole) -> Result<Self, PrintableErrno<String>> {
        let cmdline_buf = std::fs::read_to_string("/proc/cmdline").map_err(|io| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
        })?;
        let cmdline_spl = cmdline_buf.trim().split(' ');
        let mut res = Self::parse_inner(kcon, cmdline_spl)?;

        if res.root_opts.get_source().is_none() {
            res.root_opts
                .source(PartitionSourceBuilder::autodiscover_root(kcon)?);
        }

        if let Err(e) = kcon.disable_throttling_on_verbose() {
            kinfo!(kcon, "{}", e);
        }

        Ok(res)
    }

    /// The `init` binary to exec after mounting the system root. Defaults to
    /// [`/sbin/init`][crate::INIT_DEFAULT_PATH].
    ///
    /// Use parameter `init` to change this value. Example:
    ///
    /// ```no_check
    /// init=<PATH>
    /// ```
    pub fn init(&self) -> &CStr {
        &self.init[..]
    }

    /// Options necessary for mounting the root filesystem, such as partition UUID, filesystem
    /// type, flags, ...
    ///
    /// Many parameters can affect this value:
    /// - `root` to specify the root partitions device path (`/dev/sda1`), UUID
    /// (`UUID=e0805d9f-8660-431d-9cfd-134161a9f1c1`), Label (`LABEL=system_a`), GPT Label
    /// (`PARTLABEL=sysa`), GPT UUID (`PARTUUID=ff5fc4ff-4ff0-4364-9ca0-f75f85b9117d`), or
    /// GPT UUID with PARTNROFF (`PARTUUID=ff5fc4ff-4ff0-4364-9ca0-f75f85b9117d/PARTNROFF=2`).
    /// Optional, but if not given, root will be discovered through GPT partition autodiscovery
    /// and should follow the architecture's appropriate guidelines in order for it to be
    /// recognized. If no partition is found (or the root parameter specifies an invalid or
    /// nonexistent one), boot fails.
    /// - `rootfstype` to specify the filesystem type (e.g. `ext4`). Optional.
    /// - `rootflags` to specify flags for mounting the filesystem (e.g. `relatime,diratime`).
    /// Optional.
    /// - `ro` to mount the root filesystem as read-only. Note that your system may remount
    /// itself as writable after transitioning to it and `ro` will do nothing to prevent it from
    /// happening. Optional.
    /// - `rw` to mount the root filesystem as writable. Optional.
    ///
    /// Example:
    ///
    /// ```no_check
    /// root=LABEL=chimera rootfstype=btrfs rw
    /// ```
    pub fn root_opts(&self) -> &RootOptsBuilder {
        &self.root_opts
    }

    /// The swap partition used by the system to resume from hibernation.
    ///
    /// Use parameter `resume` to set this value. Can be a device path
    /// (`/dev/sda1`), UUID (`UUID=e0805d9f-8660-431d-9cfd-134161a9f1c1`), Label
    /// (`LABEL=system_a`), GPT Label (`PARTLABEL=sysa`), GPT UUID
    /// (`PARTUUID=ff5fc4ff-4ff0-4364-9ca0-f75f85b9117d`), or GPT UUID with PARTNROFF
    /// (`PARTUUID=ff5fc4ff-4ff0-4364-9ca0-f75f85b9117d/PARTNROFF=2`). Example:
    ///
    /// ```no_check
    /// resume=UUID=e0805d9f-8660-431d-9cfd-134161a9f1c1
    /// ```
    pub fn resume_source(&self) -> Option<&PartitionSourceBuilder> {
        self.resume_source.as_ref()
    }

    /// Parameters for kernel module initialization.
    ///
    /// Format for expressing in command-line arguments is `module.key = value`. Example:
    ///
    /// ```no_check
    /// amdgpu.dpm=0
    /// ```
    pub fn mod_params(&self) -> &ModParams {
        &self.mod_params
    }

    fn parse_inner<'a>(
        kcon: &mut KConsole,
        cmdline_spl: impl Iterator<Item = &'a str>,
    ) -> Result<Self, PrintableErrno<String>> {
        let mut kmsg_buf = KmsgBuf::new(kcon);
        let mut verbosity_level: Option<VerbosityLevel> = None;
        let mut init: Option<CString> = None;
        let mut root_opts = RootOpts::builder();
        let mut resume_source: Option<PartitionSourceBuilder> = None;
        let mut mod_params = ModParams::default();
        for arg in cmdline_spl {
            let (arg_key, arg_value) = match arg.split_once('=') {
                Some((ak, av)) => (ak, Some(av)),
                None => (arg, None),
            };

            match arg_key {
                "ignited.log" => {
                    Self::parse_ignited_log(&mut kmsg_buf, &mut verbosity_level, arg_value, false)
                }
                "booster.log" => {
                    Self::parse_ignited_log(&mut kmsg_buf, &mut verbosity_level, arg_value, true)
                }
                "booster.debug" => Self::parse_booster_debug(&mut kmsg_buf, &mut verbosity_level),
                "quiet" => Self::parse_quiet(&mut verbosity_level),
                "root" => Self::parse_root(&mut kmsg_buf, &mut root_opts, arg_value)?,
                "resume" => Self::parse_resume(&mut kmsg_buf, &mut resume_source, arg_value)?,
                "init" => Self::parse_init(&mut kmsg_buf, &mut init, arg_value)?,
                "rootfstype" => Self::parse_rootfstype(&mut kmsg_buf, &mut root_opts, arg_value),
                "rootflags" => Self::parse_rootflags(&mut kmsg_buf, &mut root_opts, arg_value),
                "ro" => Self::parse_rootmode(&mut root_opts, false),
                "rw" => Self::parse_rootmode(&mut root_opts, true),
                "rd.luks.options" => Self::parse_luksopts(&mut kmsg_buf),
                "rd.luks.name" => Self::parse_luksname(&mut kmsg_buf),
                "rd.luks.uuid" => Self::parse_luksuuid(&mut kmsg_buf),
                mod_param => {
                    Self::parse_mod_param(&mut kmsg_buf, &mut mod_params, mod_param, arg_value)
                }
            }
        }
        kmsg_buf.flush_with_level(verbosity_level.unwrap_or_default());
        Ok(CmdlineArgs {
            init: init.unwrap_or_else(|| INIT_DEFAULT_PATH.into()),
            root_opts,
            resume_source,
            mod_params,
        })
    }

    /// (DEPRECATED) `booster.debug` sets the logging verbosity level to Debug.
    ///
    /// This option is deprecated. Use `ignited.log=debug` instead.
    fn parse_booster_debug(kmsg_buf: &mut KmsgBuf, verbosity_level: &mut Option<VerbosityLevel>) {
        verbosity_level.get_or_insert(VerbosityLevel::Debug);
        kmsg_buf.kdebug("booster.debug is deprecated: use ignited.log=debug instead.".to_string());
    }

    /// `ignited.log=<VALUE>` and `booster.log=<VALUE-1>[,<VALUE-2>[,<...>]]` sets the
    /// logging verbosity to the specified value.
    ///
    /// - `ignited.log=<VALUE>` is preferred, where `<VALUE>` corresponds to a textual
    /// representation of a [VerbosityLevel] (see its documentation for more details).
    /// - `booster.log=<VALUE-1>[,<VALUE-2>[,<...>]]` is accepted, where:
    ///   - `<VALUE-N>` corresponds to a textual representation of a [VerbosityLevel]
    /// (see its documentation for more details).
    ///   - In case of conflicting values, the first specified value takes precedence.
    ///   - The `console` value is ignored by ignited.
    fn parse_ignited_log(
        kmsg_buf: &mut KmsgBuf,
        verbosity_level: &mut Option<VerbosityLevel>,
        arg_value: Option<&str>,
        compat: bool,
    ) {
        let (key, iter_arg_opt) = if compat {
            (
                "booster.log",
                arg_value.map(|s| {
                    s.split(',')
                        .filter(|v| !v.is_empty())
                        .collect::<Vec<&str>>()
                }),
            )
        } else {
            ("ignited.log", arg_value.map(|s| vec![s]))
        };

        if let Some(iter_arg) = iter_arg_opt {
            for arg_value in iter_arg {
                if let Ok(level) = VerbosityLevel::try_from(arg_value) {
                    verbosity_level.get_or_insert(level);
                } else if arg_value == "console" {
                    // no-op
                    kmsg_buf.kdebug(format!("{}=console is ignored in ignited", key))
                } else {
                    kmsg_buf.kwarn(format!("unknown {} key {}", key, arg_value));
                }
            }
        } else {
            kmsg_buf.kwarn(format!("unknown {} key <EMPTY>", key));
        }
    }

    /// `init=<PATH>` sets the path of the init binary to execute when handing off to the
    /// mounted system.
    fn parse_init(
        kmsg_buf: &mut KmsgBuf,
        init: &mut Option<CString>,
        arg_value: Option<&str>,
    ) -> Result<(), PrintableErrno<String>> {
        if let Some(arg_value) = arg_value {
            let new_init = CString::new(arg_value).map_err(|_| {
                printable_error(
                    PROGRAM_NAME,
                    format!("invalid init path {}: path contains null value", arg_value),
                )
            })?;
            init.get_or_insert(new_init);
        } else {
            kmsg_buf.kwarn("init key is empty, ignoring".to_string());
        }
        Ok(())
    }

    /// `<module>.<key>=<VALUE>` sets a kernel module parameter.
    fn parse_mod_param(
        kmsg_buf: &mut KmsgBuf,
        mod_params: &mut ModParams,
        mod_param: &str,
        arg_value: Option<&str>,
    ) {
        if let Some(arg_value) = arg_value {
            if let Some((module, param)) = mod_param.split_once('.') {
                mod_params.insert(module, param, arg_value);
            } else {
                kmsg_buf.kwarn(format!("invalid key {}", mod_param));
            }
        } else {
            kmsg_buf.kwarn(format!("invalid key {}", mod_param));
        }
    }

    /// `resume=<VALUE>` sets the swap partition from which to resume hibernation.
    ///
    /// See [PartitionSourceBuilder] for details on how this parameter should be formatted.
    fn parse_resume(
        kmsg_buf: &mut KmsgBuf,
        resume_source: &mut Option<PartitionSourceBuilder>,
        arg_value: Option<&str>,
    ) -> Result<(), PrintableErrno<String>> {
        if let Some(arg_value) = arg_value {
            resume_source.get_or_insert(
                PartitionSourceBuilder::parse(arg_value)
                    .ok_or_else(|| printable_error(PROGRAM_NAME, "unable to parse resume key"))?,
            );
        } else {
            kmsg_buf.kwarn("resume key is empty, ignoring".to_string());
        }
        Ok(())
    }

    /// `root=<VALUE>` sets the root partition to mount.
    ///
    /// See [PartitionSourceBuilder] for details on how this parameter should be formatted.
    fn parse_root(
        kmsg_buf: &mut KmsgBuf,
        root_opts: &mut RootOptsBuilder,
        arg_value: Option<&str>,
    ) -> Result<(), PrintableErrno<String>> {
        if let Some(arg_value) = arg_value {
            root_opts.source(
                PartitionSourceBuilder::parse(arg_value)
                    .ok_or_else(|| printable_error(PROGRAM_NAME, "unable to parse root key"))?,
            );
        } else {
            kmsg_buf.kwarn("root key is empty, ignoring".to_string());
        }
        Ok(())
    }

    /// `rootfstype=<VALUE>` sets the root partition filesystem type.
    ///
    /// See [RootOptsBuilder] for more information.
    fn parse_rootfstype(
        kmsg_buf: &mut KmsgBuf,
        root_opts: &mut RootOptsBuilder,
        arg_value: Option<&str>,
    ) {
        if let Some(arg_value) = arg_value {
            root_opts.fstype(arg_value);
        } else {
            kmsg_buf.kwarn("rootfstype key is empty, ignoring".to_string());
        }
    }

    /// `rootflags=<VALUE>` sets the root partition filesystem flags.
    ///
    /// See [RootOptsBuilder] for more information.
    fn parse_rootflags(
        kmsg_buf: &mut KmsgBuf,
        root_opts: &mut RootOptsBuilder,
        arg_value: Option<&str>,
    ) {
        if let Some(arg_value) = arg_value {
            root_opts.add_opts(arg_value);
        } else {
            kmsg_buf.kwarn("rootflags key is empty, ignoring".to_string());
        }
    }

    /// `ro` and `rw` set whether the root partition is to be initially mounted as "read-only"
    /// or "writable" respectively.
    ///
    /// See [RootOptsBuilder] for more information.
    fn parse_rootmode(root_opts: &mut RootOptsBuilder, rw: bool) {
        if rw {
            root_opts.rw();
        } else {
            root_opts.ro();
        }
    }

    fn parse_luksopts(_kmsg_buf: &mut KmsgBuf) {
        todo!("Parse luks options")
    }

    fn parse_luksname(_kmsg_buf: &mut KmsgBuf) {
        todo!("Parse luks options")
    }

    fn parse_luksuuid(_kmsg_buf: &mut KmsgBuf) {
        todo!("Parse luks options")
    }

    /// `quiet` sets the logging verbosity level to Err.
    ///
    /// ignited performs the equivalent to `ignited.log=err` when encountering the `quiet`
    /// parameter. However other userspace programs might react to `quiet` by i.e. showing
    /// a splash screen, while not reacting to `ignited.log=err` at all. Therefore `quiet`
    /// has a legitimate use. If a different ignited verbosity level is required while still
    /// using the `quiet` parameter, make sure to position `ignited.log=<VALUE>` before
    /// `quiet` like so:
    ///
    /// ```no_check
    /// root=LABEL=system_a ignited.log=debug quiet ...
    /// ```
    ///
    /// This is important as the first parameter that sets a verbosity level takes precedence in
    /// ignited over the others.
    fn parse_quiet(verbosity_level: &mut Option<VerbosityLevel>) {
        verbosity_level.get_or_insert(VerbosityLevel::Err);
    }
}
