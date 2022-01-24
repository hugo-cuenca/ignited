///! TODO
use crate::{KConsole, INIT_PATH, PROGRAM_NAME};
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
        self.console.as_ref().map(|c| ConsoleConfig(c))
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

/// TODO Move to KConsole
pub enum VerbosityLevel {
    Debug,
    Info,
    Notice,
    Warn,
    Err,
}
impl VerbosityLevel {
    fn from(level: &str) -> Option<VerbosityLevel> {
        match level {
            "debug" => Some(VerbosityLevel::Debug),
            "info" => Some(VerbosityLevel::Info),
            "notice" => Some(VerbosityLevel::Notice),
            "warn" | "warning" => Some(VerbosityLevel::Warn),
            "err" | "error" => Some(VerbosityLevel::Err),
            _ => None,
        }
    }
}
impl Default for VerbosityLevel {
    fn default() -> Self {
        VerbosityLevel::Info
    }
}

#[derive(Debug, Clone)]
pub struct CmdlineArgs {
    verbosity_level: VerbosityLevel,
    init: CString,
    root_fstype: Option<String>,
    module_params: BTreeMap<String, String>,
}
impl CmdlineArgs {
    pub fn parse_current(kcon: &KConsole) -> Result<Self, PrintableErrno<String>> {
        let cmdline_buf = BufReader::new(File::open(Path::new("/proc/cmdline")));
        let cmdline_spl = cmdline_buf.split(b' ');
        Self::parse_inner(kcon, cmdline_spl)
    }

    fn parse_inner<B: BufRead>(
        kcon: &KConsole,
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

        let mut verbosity_level: Option<VerbosityLevel> = None;
        let mut init: Option<CString> = None;
        let mut root_fstype: Option<String> = None;
        let mut module_params = BTreeMap::new();
        for arg in cmdline_spl {
            let arg = arg.map_err(|io| {
                printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
            })?;
            let arg = try_or_cont!(std::str::from_utf8(&arg[..]));

            let (arg_key, arg_value) = match arg.split_once('=') {
                Some((ak, av)) => (ak, Some(av)),
                None => (arg, None),
            };

            // TODO
            match arg_key {
                "ignited.log" => {
                    if let Some(level) = arg_value.map(|v| VerbosityLevel::from(v)).flatten() {
                        verbosity_level.get_or_insert(level);
                    } else {
                        kwarning!(
                            kcon,
                            "unknown ignited.log key {}",
                            arg_value.unwrap_or("<EMPTY>")
                        );
                    }
                }
                "booster.log" => {
                    // COMPAT ARG. TODO DOCUMENT DIFFERENCES
                    if let Some(arg_value) = arg_value {
                        for arg_value in arg_value.split(',').filter(|v| !v.is_empty()) {
                            if let Some(level) = VerbosityLevel::from(arg_value) {
                                verbosity_level.get_or_insert(level);
                            } else if arg_value == "console" {
                                // no-op
                                kdebug!(kcon, "booster.log=console is ignored in ignited");
                            } else {
                                kwarning!(kcon, "unknown booster.log key {}", arg_value);
                            }
                        }
                    } else {
                        kwarning!(kcon, "unknown booster.log key <EMPTY>");
                    }
                }
                "booster.debug" => {
                    verbosity_level.get_or_insert(VerbosityLevel::Debug);
                    kdebug!(
                        kcon,
                        "booster.debug is deprecated: use ignited.log=debug instead."
                    );
                }
                "quiet" => {
                    verbosity_level.get_or_insert(VerbosityLevel::Err);
                }
                "root" => {
                    todo!("Parse root")
                }
                "resume" => {
                    todo!("Parse resume")
                }
                "init" => {
                    if let Some(arg_value) = arg_value {
                        let new_init = CString::new(arg_value).map_err(|_| {
                            printable_error(
                                PROGRAM_NAME,
                                format!(
                                    "invalid init path {}: path contains null value",
                                    arg_value
                                ),
                            )
                        })?;
                        init.get_or_insert(new_init);
                    } else {
                        kwarning!(kcon, "init key is empty, ignoring");
                    }
                }
                "rootfstype" => {
                    if let Some(arg_value) = arg_value {
                        root_fstype.get_or_insert(arg_value.to_string())
                    } else {
                        kwarning!(kcon, "rootfstype key is empty, ignoring");
                    }
                }
                "rootflags" => {
                    todo!("Parse rootflags")
                }
                "ro" => {
                    todo!("Parse rootflags")
                }
                "rw" => {
                    todo!("Parse rootflags")
                }
                "rd.luks.options" => {
                    todo!("Parse luks options")
                }
                "rd.luks.name" => {
                    todo!("Parse luks options")
                }
                "rd.luks.uuid" => {
                    todo!("Parse luks options")
                }
                module_param => {
                    if let Some(arg_value) = arg_value {
                        if let Some((module, param)) = module_param.split_once('.') {
                            module_params.insert(
                                module.replace('-', "_"),
                                format!("{}={}", param, arg_value),
                            )
                        } else {
                            kwarning!(kcon, "invalid key {}", module_param);
                        }
                    } else {
                        kwarning!(kcon, "invalid key {}", module_param);
                    }
                }
            }
        }
        Ok(CmdlineArgs {
            verbosity_level: verbosity_level.unwrap_or_default(),
            init: init.unwrap_or_else(|| INIT_PATH.into_c_string()),
            root_fstype,
            module_params,
        })
    }
}
