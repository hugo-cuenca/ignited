//! Kernel module loading.
//!
//! This (rust code) module allows detecting and loading of (kernel) modules contained
//! in the initramfs. In the future, further (kernel) modules may be loaded in a
//! special `/vendor` partition.

use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::collections::BTreeMap;

/// (Kernel) Module alias.
///
/// Inside of `/sys/devices/*` there is a `modalias` file for every device with a
/// loadable kernel module. `/usr/lib/modules/ignited.alias` should contain all alias
/// patterns that correspond to a kernel module to be loaded from the initramfs.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct ModAlias {
    pattern: String,
    module: String,
}
impl ModAlias {
    /// Create a new `ModAlias` containing a pattern and a kernel module. When a
    /// kernel module for a device wants to be loaded, the device's `modalias`
    /// should be matched with the `ModAlias`'s pattern first. On success, the
    /// kernel module should then be loaded.
    pub fn new(pattern: String, module: String) -> Self {
        Self { pattern, module }
    }
}

/// List of (kernel) module aliases.
///
/// `/usr/lib/modules/ignited.alias` should contain all of the module aliases in
/// the following format:
///
/// ```no_run
/// PATTERN MODULE
/// PATTERN MODULE
/// PATTERN MODULE
/// ...
/// ```
#[derive(Debug, Default, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ModAliases(Vec<ModAlias>);
impl Extend<ModAlias> for ModAliases {
    fn extend<T: IntoIterator<Item = ModAlias>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}
impl TryFrom<std::fs::File> for ModAliases {
    type Error = PrintableErrno<String>;

    fn try_from(value: std::fs::File) -> Result<Self, Self::Error> {
        use std::io::{BufRead, BufReader};

        let reader = BufReader::new(value);
        let lines = reader.lines();
        let mut result = Vec::new();
        for line_result in lines {
            let line = line_result.map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while reading module aliases: {}", io),
                )
            })?;
            let (pattern, module) = line.split_once(" ").ok_or_else(|| {
                printable_error(
                    PROGRAM_NAME,
                    "error while reading module aliases: missing whitespace",
                )
            })?;
            result.push(ModAlias::new(pattern.to_string(), module.to_string()))
        }

        Ok(ModAliases(result))
    }
}
impl TryFrom<&std::path::Path> for ModAliases {
    type Error = PrintableErrno<String>;

    fn try_from(value: &std::path::Path) -> Result<Self, Self::Error> {
        ModAliases::try_from(std::fs::File::open(value).map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("error while reading module aliases: {}", io),
            )
        })?)
    }
}

/// List of parameters to be passed to (kernel) module initialization.
#[derive(Debug, Default, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ModParams(BTreeMap<String, Vec<String>>);
impl ModParams {
    /// Get the parameters to be passed to the module when initialized.
    #[inline]
    pub fn get_params<M: AsRef<str>>(&self, module: M) -> &[String] {
        self._get_params(module.as_ref())
    }

    /// Insert a new parameter to be passed to the module when initialized.
    #[inline]
    pub fn insert<M: AsRef<str>, P: AsRef<str>, A: AsRef<str>>(
        &mut self,
        module: M,
        param: P,
        args: A,
    ) {
        self._insert(module.as_ref(), param.as_ref(), args.as_ref())
    }

    /// Normalize module name.
    ///
    /// Module names use underscores instead of dashes, yet dashes are specified
    /// in the command line boot arguments. This function changes the dashes
    /// in the string to underscores.
    pub fn normalize_module(module: &str) -> String {
        module.replace('-', "_")
    }

    fn _get_params(&self, module: &str) -> &[String] {
        self.0
            .get(&Self::normalize_module(module))
            .map(|a| &a[..])
            .unwrap_or_default()
    }

    fn _insert(&mut self, module: &str, param: &str, args: &str) {
        self.0
            .entry(Self::normalize_module(module))
            .or_default()
            .push(format!("{}={}", param, args));
    }
}
