//! Kernel module loading.
//!
//! This (rust code) module allows detecting and loading of (kernel) modules contained
//! in the initramfs. In the future, further (kernel) modules may be loaded in a
//! special `/vendor` partition.

use crate::{
    early_logging::KConsole, CmdlineArgs, RuntimeConfig, IGNITED_KERN_MODULES,
    PROGRAM_NAME,
};
use crossbeam_utils::sync::WaitGroup;
use dashmap::DashSet;
use goglob::GlobPattern;
use nix::kmod::{finit_module, ModuleInitFlags};
use precisej_printable_errno::{printable_error, ErrnoResult, PrintableErrno};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    ffi::CString,
    fs::File,
    ops::DerefMut,
    sync::{Arc, Mutex},
    thread,
};

/// (Kernel) Module alias.
///
/// Inside of `/sys/devices/*` there is a `modalias` file for every device with a
/// loadable kernel module. `/usr/lib/modules/ignited.alias` should contain all alias
/// patterns that correspond to a kernel module to be loaded from the initramfs.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ModAlias {
    pattern: GlobPattern,
    module: String,
}
impl ModAlias {
    /// Create a new `ModAlias` containing a pattern and a kernel module. When a
    /// kernel module for a device wants to be loaded, the device's `modalias`
    /// should be matched with the `ModAlias`'s pattern first. On success, the
    /// kernel module should then be loaded.
    pub fn new(pattern: GlobPattern, module: String) -> Self {
        Self { pattern, module }
    }

    /// Match a given alias with the pattern, returning the associated kernel module
    /// if successful.
    pub fn match_alias<S: AsRef<str>>(&self, alias: S) -> Result<&str, ()> {
        self._match_alias(alias.as_ref())
    }
    fn _match_alias<'a>(&'a self, alias: &'_ str) -> Result<&'a str, ()> {
        self.pattern.matches(alias).then(|| &*self.module).ok_or(())
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
#[derive(Debug, Default, Clone)]
pub struct ModAliases {
    bookkeeping_processed: Arc<DashSet<String>>,
    aliases: Arc<Vec<ModAlias>>,
}
impl ModAliases {
    /// Match the modules that correspond with the given alias
    pub fn match_alias<S: Into<String>>(
        &self,
        alias: S,
        modules: &mut Vec<String>,
    ) -> Result<(), PrintableErrno<String>> {
        self._match_alias(alias.into(), modules)
    }
    fn _match_alias(
        &self,
        alias: String,
        modules: &mut Vec<String>,
    ) -> Result<(), PrintableErrno<String>> {
        if self.bookkeeping_processed.insert(alias.clone()) {
            // Hasn't been processed yet
            for available_alias in self.aliases.as_ref() {
                if let Ok(module) = available_alias.match_alias(&alias) {
                    modules.push(module.to_string())
                }
            }
        }

        Ok(())
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
            let pattern = GlobPattern::new(pattern).map_err(|e| {
                printable_error(
                    PROGRAM_NAME,
                    format!(
                        "error while reading module aliases: malformed pattern: {}", e
                    ),
                )
            })?;
            result.push(ModAlias::new(pattern, module.to_string()))
        }

        Ok(ModAliases {
            bookkeeping_processed: Arc::new(DashSet::new()),
            aliases: Arc::new(result),
        })
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
    fn _get_params(&self, module: &str) -> &[String] {
        self.0
            .get(&Self::normalize_module(module))
            .map(|a| &a[..])
            .unwrap_or_default()
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
    fn _insert(&mut self, module: &str, param: &str, args: &str) {
        self.0
            .entry(Self::normalize_module(module))
            .or_default()
            .push(format!("{}={}", param, args));
    }

    /// Normalize module name.
    ///
    /// Module names use underscores instead of dashes, yet dashes are specified
    /// in the command line boot arguments. This function changes the dashes
    /// in the string to underscores.
    pub fn normalize_module(module: &str) -> String {
        module.replace('-', "_")
    }
}

// Inner struct containing ModuleLoading's fields. Meant to be guarded by a mutex.
#[derive(Debug, Default)]
struct ModLoadingInner {
    loaded: BTreeMap<String, ()>,
    loading: BTreeMap<String, Vec<WaitGroup>>,
}

/// (Kernel) module loading WaitGroup.
pub struct ModWg(WaitGroup);
impl ModWg {
    /// Wait for kernel modules to finish loading.
    pub fn wait(self) {
        self.0.wait()
    }
}

/// (Kernel) module loading and bookkeeping: records already loaded modules.
#[derive(Debug, Clone)]
pub struct ModLoading {
    bookkeeping: Arc<Mutex<ModLoadingInner>>,
    config: Arc<RuntimeConfig>,
    args: Arc<CmdlineArgs>,
    aliases: ModAliases,
}
impl ModLoading {
    /// Build a new instance of this struct. This should only be called once.
    pub fn new(config: &Arc<RuntimeConfig>, args: &Arc<CmdlineArgs>, aliases: ModAliases) -> Self {
        Self {
            bookkeeping: Arc::new(Mutex::new(ModLoadingInner::default())),
            config: Arc::clone(config),
            args: Arc::clone(args),
            aliases,
        }
    }

    /// Load the (kernel) module that corresponds to the given alias.
    #[inline]
    pub fn load_modalias<S: AsRef<str>>(
        &self,
        alias: S
    ) -> Result<Option<ModWg>, PrintableErrno<String>> {
        self._load_modalias(alias.as_ref())
    }
    fn _load_modalias(&self, alias: &str) -> Result<Option<ModWg>, PrintableErrno<String>> {
        let mut modules: Vec<String> = Vec::new();
        self.aliases.match_alias(alias, &mut modules)?;
        if !modules.is_empty() {
            self.load_modules(&modules).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Load the specified (kernel) modules.
    pub fn load_modules(&self, modules: &[String]) -> Result<ModWg, PrintableErrno<String>> {
        let wg = WaitGroup::new();
        let mut unlocked = self.bookkeeping.lock().map_err(|_| {
            printable_error(PROGRAM_NAME, "unable to lock module-loading".to_string())
        })?;
        self.load_modules_unlocked(modules, &wg, unlocked.deref_mut())?;
        Ok(ModWg(wg))
    }
    fn load_modules_unlocked(
        &self,
        modules: &'_ [String],
        wg: &WaitGroup,
        unlocked: &mut ModLoadingInner,
    ) -> Result<(), PrintableErrno<String>> {
        for module in modules {
            if unlocked.loaded.contains_key(module)
                || self.config.metadata().module_builtin().contains(module)
            {
                // If module is already loaded or is built-in to the kernel, skip
                continue;
            }

            let wg_cl = wg.clone();
            match unlocked.loading.entry(module.clone()) {
                Entry::Vacant(v) => {
                    v.insert(vec![wg_cl]);
                }
                Entry::Occupied(mut o) => {
                    // wg is already incremented, so just add it to the map and continue
                    o.get_mut().push(wg_cl);
                    continue;
                }
            }

            let deps_wg = WaitGroup::new();
            if let Some(deps) = self.config.metadata().module_deps().get(module) {
                self.load_modules_unlocked(&deps[..], &deps_wg, unlocked)?;
            }

            let module = module.clone();
            let self_cl = self.clone();
            let wg_cl = wg.clone();
            thread::spawn(move || self_cl.load_module(module.as_ref(), deps_wg, wg_cl));
        }
        Ok(())
    }
    fn load_module(
        &self,
        module: &str,
        deps_wg: WaitGroup,
        orig_wg: WaitGroup,
    ) -> Result<(), PrintableErrno<String>> {
        // KConsole has been successfully opened before, so this should never fail.
        let mut kcon = KConsole::new().unwrap();

        deps_wg.wait();

        Self::finit(&mut kcon, module, &self.config, &self.args)?;
        let mut unlocked = self.bookkeeping.lock().map_err(|_| {
            printable_error(PROGRAM_NAME, "unable to lock module-loading".to_string())
        })?;
        if let Some(wgs) = unlocked.loading.remove(module) {
            for wg in wgs {
                drop(wg)
            }
        }

        if let Some(deps) = self.config.metadata().module_post_deps().get(module) {
            self.load_modules_unlocked(&deps[..], &orig_wg, unlocked.deref_mut())?;
        }
        Ok(())
    }

    /// Actually load the specified (kernel) module.
    fn finit(
        kcon: &mut KConsole,
        module: &str,
        config: &RuntimeConfig,
        args: &CmdlineArgs,
    ) -> Result<(), PrintableErrno<String>> {
        let f = File::open(format!("{}/{}.ko", IGNITED_KERN_MODULES, module)).map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!(
                    "unable to open {}/{}.ko: {}",
                    IGNITED_KERN_MODULES, module, io
                ),
            )
        })?;

        // Comment from booster:
        // I am not sure if ordering is important but we add modprobe params first and then cmdline
        let mut params = config
            .metadata()
            .module_opts()
            .get(module)
            .cloned()
            .unwrap_or_default();
        params.push_str(&format!(
            " {}",
            args.mod_params().get_params(module).join(" ")
        ));
        if params.is_empty() {
            kdebug!(kcon, "loading module {}", module);
        } else {
            kdebug!(kcon, "loading module {} params=\"{}\"", module, &params);
        }
        let params_c = CString::new(params).map_err(|_| {
            printable_error(
                PROGRAM_NAME,
                "unable to convert parameters to string".to_string(),
            )
        })?;
        finit_module(&f, params_c.as_ref(), ModuleInitFlags::empty())
            .printable(PROGRAM_NAME, format!("unable to load module {}", module))
    }
}
