//! TODO

use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::collections::BTreeMap;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct ModAlias {
    pattern: String,
    module: String,
}
impl ModAlias {
    pub fn new(pattern: String, module: String) -> Self {
        Self { pattern, module }
    }
}

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

#[derive(Debug, Default, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ModParams(BTreeMap<String, Vec<String>>);
impl ModParams {
    #[inline]
    pub fn get_params<M: AsRef<str>>(&self, module: M) -> &[String] {
        self._get_params(module.as_ref())
    }

    #[inline]
    pub fn insert<M: AsRef<str>, P: AsRef<str>, A: AsRef<str>>(
        &mut self,
        module: M,
        param: P,
        args: A,
    ) {
        self._insert(module.as_ref(), param.as_ref(), args.as_ref())
    }

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
