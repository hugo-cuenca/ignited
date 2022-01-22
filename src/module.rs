//! TODO

use crate::PROGRAM_NAME;
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

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

    fn try_from(value: File) -> Result<Self, Self::Error> {
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
