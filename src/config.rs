///! TODO
use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, path::Path};

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
impl TryFrom<std::fs::File> for RuntimeConfig {
    type Error = PrintableErrno<String>;

    #[inline(always)]
    fn try_from(mut value: std::fs::File) -> Result<Self, Self::Error> {
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
    fn try_from(value: &std::path::Path) -> Result<Self, Self::Error> {
        let out = std::fs::read_to_string(value).map_err(|io| {
            printable_error(PROGRAM_NAME, format!("error while reading config: {}", io))
        })?;
        Self::try_from(out)
    }
}
