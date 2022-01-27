///! TODO
use crate::{
    early_logging::{buf::KmsgBuf, KConsole, VerbosityLevel},
    module::ModParams,
    mount::{RootOpts, RootOptsBuilder, PartitionSourceBuilder},
    INIT_PATH, PROGRAM_NAME,
};
use precisej_printable_errno::{printable_error, PrintableErrno};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{read_to_string, File},
    io::{BufRead, BufReader, Read},
    path::Path,
};

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
#[repr(transparent)]
pub struct InitramfsMetadata<'a>(&'a InitramfsMetadataDe);
impl<'a> InitramfsMetadata<'a> {
    pub fn kernel_ver(&'_ self) -> &'_ str {
        &self.0.kernel_ver[..]
    }

    pub fn module_builtin(&'_ self) -> &'_ [String] {
        &self.0.module_builtin[..]
    }

    pub fn module_deps(&'_ self) -> &'_ BTreeMap<String, Vec<String>> {
        &self.0.module_deps
    }

    pub fn module_opts(&'_ self) -> &'_ BTreeMap<String, String> {
        &self.0.module_opts
    }

    pub fn module_post_deps(&'_ self) -> &'_ BTreeMap<String, Vec<String>> {
        &self.0.module_post_deps
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
struct IgnitedConfigDe {
    lvm: bool,
    mdraid: bool,
    module_force: Vec<String>,
    mount_timeout: Option<i64>,
}
#[repr(transparent)]
pub struct IgnitedConfig<'a>(&'a IgnitedConfigDe);
impl<'a> IgnitedConfig<'a> {
    pub fn has_lvm(&self) -> bool {
        self.0.lvm
    }

    pub fn has_mdraid(&self) -> bool {
        self.0.mdraid
    }

    pub fn get_force_modules(&'_ self) -> &'_ [String] {
        &self.0.module_force[..]
    }

    pub fn get_mount_timeout(&self) -> Option<i64> {
        self.0.mount_timeout.filter(|m| *m > 0)
    }
}

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
#[repr(transparent)]
pub struct ConsoleConfig<'a>(&'a ConsoleConfigDe);
impl<'a> ConsoleConfig<'a> {
    pub fn is_utf8(&self) -> bool {
        self.0.utf
    }

    pub fn font_file(&'_ self) -> &'_ str {
        &self.0.font_file_p[..]
    }

    pub fn font_map_file(&'_ self) -> &'_ str {
        &self.0.font_map_file_p[..]
    }

    pub fn font_unicode_file(&'_ self) -> &'_ str {
        &self.0.font_unicode_file_p[..]
    }

    pub fn keymap_file(&'_ self) -> &'_ str {
        &self.0.keymap_file_p[..]
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct RuntimeConfig {
    metadata: InitramfsMetadataDe,
    ignited: IgnitedConfigDe,
    console: Option<ConsoleConfigDe>,
}
impl RuntimeConfig {
    pub fn metadata(&self) -> InitramfsMetadata<'_> {
        InitramfsMetadata(&self.metadata)
    }

    pub fn sysconf(&self) -> IgnitedConfig<'_> {
        IgnitedConfig(&self.ignited)
    }

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

#[derive(Debug, Clone)]
pub struct CmdlineArgs {
    init: CString,
    root_opts: RootOptsBuilder,
    resume_source: Option<PartitionSourceBuilder>,
    mod_params: ModParams,
}
impl CmdlineArgs {
    pub fn parse_current(kcon: &mut KConsole) -> Result<Self, PrintableErrno<String>> {
        let cmdline_buf = BufReader::new(File::open(Path::new("/proc/cmdline")).map_err(|io| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
        })?);
        let cmdline_spl = cmdline_buf.split(b' ');
        Self::parse_inner(kcon, cmdline_spl)
    }

    fn parse_inner<B: BufRead>(
        kcon: &mut KConsole,
        cmdline_spl: std::io::Split<B>,
    ) -> Result<Self, PrintableErrno<String>> {
        macro_rules! try_or_cont {
            ($expr:expr $(,)?) => {
                match $expr {
                    ::core::result::Result::Ok(val) => val,
                    ::core::result::Result::Err(_) => {
                        continue;
                    }
                }
            };
        }

        let mut kmsg_buf = KmsgBuf::new(kcon);
        let mut verbosity_level: Option<VerbosityLevel> = None;
        let mut init: Option<CString> = None;
        let mut root_opts = RootOpts::builder();
        let mut resume_source: Option<PartitionSourceBuilder> = None;
        let mut mod_params = ModParams::default();
        for arg in cmdline_spl {
            let arg = arg.map_err(|io| {
                printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
            })?;
            let arg = try_or_cont!(std::str::from_utf8(&arg[..]));

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
            init: init.unwrap_or_else(|| INIT_PATH.into()),
            root_opts,
            resume_source,
            mod_params,
        })
    }

    fn parse_booster_debug(kmsg_buf: &mut KmsgBuf, verbosity_level: &mut Option<VerbosityLevel>) {
        verbosity_level.get_or_insert(VerbosityLevel::Debug);
        kmsg_buf.kdebug("booster.debug is deprecated: use ignited.log=debug instead.".to_string());
    }

    /// TODO document difference between compat=false/true
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

    fn parse_quiet(verbosity_level: &mut Option<VerbosityLevel>) {
        verbosity_level.get_or_insert(VerbosityLevel::Err);
    }
}
