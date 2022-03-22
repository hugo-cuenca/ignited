//! Rapid early-ramdisk system initialization, accompanying initramfs generator. **CURRENTLY IN DEVELOPMENT**
//!
//! # What?
//! ignited is a simple program meant to run before your Linux system partition has even been
//! mounted! It is intended to run inside your initramfs and bring your system to a usable
//! state, at which point it hands off execution to your `/sbin/init` program (which could be
//! systemd, initd, or any other init program). More specifically, ignited would be your
//! initramfs' `/init`.
//!
//! ignited is also the program used to generate initramfs images that contains the ignited
//! `/init` and necessary kernel modules.
//!
//! Although written in Rust, ignited is based on [Booster](https://github.com/anatol/booster)
//! which itself is written in Go. Booster is licensed under
//! [MIT](https://raw.githubusercontent.com/anatol/booster/master/LICENSE).
//!
//! ## What is an initramfs?
//! From Booster's README:
//!
//!     Initramfs is a specially crafted small root filesystem that
//!     [is] mounted at the early stages of Linux OS boot process.
//!     This initramfs, among other things, is responsible for [...]
//!     mounting [the] root filesystem.
//!
//! In other words, the initramfs is in charge of bringing the system to a usable state,
//! mounting the root filesystem, and handing off further work to your system's init.
//!
//! # Where?
//! ignited only supports Linux systems, and there are no plans to expand compatibility to
//! other OSes.
//!
//! If any Linux distribution maintainer/developer is exploring the use of ignited inside
//! their initramfs archives (or is already doing so), please contact me to be included in
//! this README. I am also able to give recommendations on proper integration in a distribution
//! if you contact me.
//!
//! # Why?
//! initramfs generation programs generally fall into 2 categories:
//!
//! - Distro-specific programs such as `mkinitcpio`.
//! - Universal, extensible programs such as `dracut`.
//!
//! The former is generally tied to a specific Linux distribution (such as Arch Linux) and
//! will not function properly outside of it without modification. The latter eschews specific
//! behaviors in favor of widely-used, mostly distro-agnostic components (such as systemd or
//! busybox). In most cases, neither are specifically designed for boot time performance nor
//! image-generation performance. Neither do they strive to be self-contained.
//!
//! ignited seeks to fulfill three goals:
//!
//! - Fast image-generation and boot time.
//! - Compatibility with as many distributions as possible (including ones without systemd,
//!   busybox, or even udevd).
//! - Contain as close to no external dependencies as possible.
//!
//! At the time of this writing, not all goals have been fulfilled. See `GOALS.md` in the
//! git repo for what progress needs to be made.
//!
//! ## Why not dracut?
//! - `dracut` doesn't optimize for image-generation speed, as evidenced by
//!   [this post by Michael Stapelberg](https://michael.stapelberg.ch/posts/2020-01-21-initramfs-from-scratch-golang/)
//!   in which he recorded ~31s for dracut image generation with `gzip` (lowered to ~9s with
//!   `pigz`).
//! - While `dracut` is compatible with many distributions (such as Fedora/RHEL, Debian,
//!   Arch Linux, openSUSE, among others), its upstream default hooks rely on systemd and/or
//!   busybox. Theoretically, it seems that special hooks could be made for non-standard
//!   configurations, but they don't seem to be explicitly supported.
//! - While `systemd` might be statically compiled with musl, such configuration is non-standard
//!   and requires various patches. `dracut`'s upstream default hooks also bundle many scripts
//!   to be executed at boot time, thus requiring additional files and interpreters (plus its
//!   dynamic link and their libraries). Additionally, image-generation consists on running
//!   various scripts and binaries to build the image, which may pull in other dependencies.
//!
//! ## Why not mkinitcpio?
//! - Image-generation takes between 10-20 seconds on my AMD Threadripper 3960X with `mkinitcpio`.
//! - `mkinitcpio` is specifically designed for Arch Linux and derivatives.
//! - Two options are given for initramfs generation by default: `udev+busybox`, or `systemd`,
//!   both of which come with the same problems as described in `Why not dracut?`. Additionally,
//!   `mkinitcpio` makes use of hooks (similarly to `dracut`), therefore inheriting its drawbacks.
//!
//! ## Why not booster?
//! ignited is based on `booster`, therefore inheriting its benefits over other initramfs
//! generators. As time progresses, ignited will differentiate itself from `booster` with specific
//! features such as verified root, A/B system partitions, and `/vendor` partition support.
//!
//! # How?
//! While all code should be properly documented, `docs.rs` currently doesn't support automatic
//! rustdoc generation for binaries. Work is being done to remedy this and provide proper generated
//! documentation. In the meantime you can browse the source code.
//!
//! `ignited(1)` and `ignited(8)` man pages will be made in the future describing the initramfs'
//! `/init` behavior and the initramfs generator respectively.
//!
//! # initd?
//! initd is a simple, serviced-compatible init implementation that is perfectly suitable for
//! systems with ignited-generated initramfs. ignited can handoff further execution to initd after
//! the root filesystem has been mounted. You can learn more on
//! [initd's git repo](https://github.com/hugo-cuenca/initd). Note that having initd installed is
//! not necessary to use ignited, as it can function just as well if your system uses systemd,
//! runit, dinit, OpenRC, or whatever init program you choose to use.
#![crate_name = "ignited"]
// #![cfg_attr(test, deny(warnings))] // TODO
// #![deny(unused)] // TODO
#![deny(unstable_features)]
#![deny(missing_docs)]
#![allow(rustdoc::private_intra_doc_links)]

#[macro_use]
mod early_logging;

mod common;
mod config;
mod module;
mod mount;
mod sysfs;
mod time;
mod udev;
mod util;
mod vconsole;

use crate::{
    config::{CmdlineArgs, InitramfsMetadata, RuntimeConfig},
    early_logging::KConsole,
    module::{ModAliases, ModLoading},
    mount::{Mount, TmpfsOpts},
    sysfs::SysfsWalker,
    time::InitramfsTimer,
    udev::UdevListener,
    util::{get_booted_kernel_ver, make_shutdown_pivot_dir, spawn_emergency_shell},
    vconsole::setup_vconsole,
};
use cstr::cstr;
use mio::{Events, Poll, Token, Waker};
use nix::{
    mount::MsFlags,
    unistd::{execv, sync},
};
use precisej_printable_errno::{
    printable_error, ErrnoResult, ExitError, ExitErrorResult, PrintableErrno, PrintableResult,
};
use std::{
    ffi::{CStr, OsStr},
    hint::unreachable_unchecked,
    io::ErrorKind,
    path::Path,
    process::id as getpid,
    sync::Arc,
    time::Duration,
};

/// The program is called `ignited`. The str referring to the program name is saved in
/// this constant. Useful for PrintableResult.
const PROGRAM_NAME: &str = "ignited";

/// Path where init is normally located. Used in the `execv` call to actually
/// execute init. The boot-time parameter `init=<PATH>` will replace this default
/// with `<PATH>`.
///
/// **Note**: if you are a distribution maintainer, make sure your
/// `initd`/`systemd`/`dinit`/whatever package actually puts the `init` executable
/// in `/sbin/init`. Otherwise, you must maintain a patch changing `INIT_PATH` to
/// the appropriate path (e.g. `/init`, `/bin/init`, or `/usr/bin/init`).
const INIT_DEFAULT_PATH: &CStr = cstr!("/sbin/init");

/// Error message used in case `INIT_PATH` is not able to be executed by `execv`.
/// This can be caused by not having init installed in the right path with the
/// proper executable permissions.
const INIT_ERROR: &str = "unable to execute init";

/// Path where `ignited`'s config file is located.
///
/// See [RuntimeConfig] for the structure of the TOML file.
const IGNITED_CONFIG: &str = "/etc/ignited/engine.toml";

/// Path where `ignited`'s module aliases file is located.
///
/// See [ModAliases] for the structure of the file.
const IGNITED_MODULE_ALIASES: &str = "/usr/lib/modules/ignited.alias";

/// Path where `ignited`'s (kernel) modules are located.
///
/// See [ModAliases] for the structure of the file.
const IGNITED_KERN_MODULES: &str = "/usr/lib/modules";

/// Ignited main thread event loop waker.
const IGNITED_MAIN_THREAD_WAKE_TOKEN: Token = Token(10);

/// Check to see if we are running as the `init` inside the initramfs.
///
/// - Make sure we are PID 1.
/// - Check for the existence of `/etc/initrd-release` (see
/// [INITRD_INTERFACE](https://systemd.io/INITRD_INTERFACE/)).
fn initial_sanity_check() -> Result<(), PrintableErrno<String>> {
    // We must be the initramfs' PID1
    (getpid() == 1).then(|| ()).ok_or_else(|| {
        printable_error(
            PROGRAM_NAME,
            "not running in an initrd environment, exiting...",
        )
    })?;

    // Per https://systemd.io/INITRD_INTERFACE/, we should only run if /etc/initrd-release
    // is present
    Path::new("/etc/initrd-release")
        .exists()
        .then(|| ())
        .ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                "not running in an initrd environment, exiting...",
            )
        })?;

    Ok(())
}

/// Perform initial work.
///
/// - Mount `/dev` as `devtmpfs`.
/// - Open `/dev/kmsg` for writing.
fn initialize_kcon() -> Result<KConsole, PrintableErrno<String>> {
    Mount::DevTmpfs.mount()?;

    // /dev should be mounted at this point
    let kcon = KConsole::new()?;
    Ok(kcon)
}

/// Check if booted kernel version matches initramfs kernel version.
///
/// The current initramfs [RuntimeConfig] contains the kernel version it was built for.
/// To prevent a module version mismatch, check if the current booted kernel version
/// matches the one in the config.
fn kernel_ver_check(config: InitramfsMetadata) -> Result<(), PrintableErrno<String>> {
    let cur_ver = &get_booted_kernel_ver()[..];
    let conf_ver = config.kernel_ver();
    (cur_ver == conf_ver)
        .then(|| ())
        .ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                format!(
                    "Linux kernel version mismatch. This initramfs image was built for version {con} and it is incompatible with the currently running version {cur}. Please rebuild the ignited image for kernel {cur}.",
                    con = conf_ver,
                    cur = cur_ver,
                ),
            )
        })
}

/// The entry point of the program. This function is in charge of exiting with an error
/// code when [init] returns an [ExitError].
fn main() {
    // immediately start timer
    let timer = InitramfsTimer::start();

    initial_sanity_check().bail(1).unwrap_or_eprint_exit();
    let mut kcon = initialize_kcon().bail(2).unwrap_or_eprint_exit();

    // Note that, although KConsole is open, no logging level is set yet.
    // Wait until it's set (with CmdlineArgs::parse_current) before logging...
    if let Err(e) = init(&mut kcon, timer) {
        kcrit!(kcon, "{}", &e);
        spawn_emergency_shell(&mut kcon).unwrap_err();
        kcrit!(kcon, "unable to spawn emergency shell");
        kcrit!(kcon, "syncing disks");
        sync();
        kcrit!(kcon, "finished syncing disks, kernel will panic on exit");
        e.eprint_and_exit()
    }
}

/// Here is where it actually begins.
///
/// - Mount `/sys`, `/proc`, and `/run` in that order.
/// - If in EFI mode, mount `/sys/firmware/efi/efivars`.
/// - Set path to a sensible default: `/usr/sbin:/usr/bin:/sbin:/bin`.
/// - Read the current [RuntimeConfig] and [ModAliases].
/// - Parse command line arguments.
/// - Create the `/run/initramfs` directory as per
///   [systemd's INITRD_INTERFACE](https://systemd.io/INITRD_INTERFACE/).
/// - Listen to udev events helpful to finding and mounting the root
///   partition at `/system_root`.
/// - Load required modules.
/// - Walk the `sysfs` filesystem to attempt to find and mount the root
///   partition at `/system_root`.
/// - Wait (optionally with a timeout) until the target root filesystem is
///   mounted properly at `/system_root`.
/// - Switch to the target root filesystem.
/// - Transition to the target's init executable at [INIT_DEFAULT_PATH]
///   (usually `/sbin/init`).
fn init(kcon: &mut KConsole, timer: InitramfsTimer) -> Result<(), ExitError<String>> {
    // Commence ignition
    Mount::Sysfs.mount().bail(3)?;
    Mount::Proc.mount().bail(3)?;
    Mount::Tmpfs(TmpfsOpts::new(
        "run",
        Path::new("/run"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_STRICTATIME,
        Some("mode=755"),
    ))
    .mount()
    .bail(3)?;

    // If we are booted in EFI mode, we should mount efivarfs
    let efi_mode = Path::new("/sys/firmware/efi").exists();
    if efi_mode {
        Mount::Efivarfs.mount().bail(3)?;
    }

    std::env::set_var("PATH", OsStr::new("/usr/sbin:/usr/bin:/sbin:/bin")); // Panics on error

    let config = Arc::new(RuntimeConfig::try_from(Path::new(IGNITED_CONFIG)).bail(4)?);
    kernel_ver_check(config.metadata()).bail(5)?;

    let aliases = ModAliases::try_from(Path::new(IGNITED_MODULE_ALIASES)).bail(6)?;
    make_shutdown_pivot_dir().bail(7)?;

    let args = Arc::new(CmdlineArgs::parse_current(kcon).bail(8)?);

    // KConsole logging level is now set, start logging here.
    timer.log(kcon);
    if efi_mode {
        kdebug!(kcon, "booted in efi mode");
    } else {
        kdebug!(kcon, "booted in bios/legacy mode");
    }

    let mod_loading = ModLoading::new(&config, &args);

    let mut evloop = Poll::new()
        .map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("error while setting up main event loop: {}", io),
            )
        })
        .bail(9)?;
    let mut evs = Events::with_capacity(2);

    let main_waker = Arc::new(
        Waker::new(evloop.registry(), IGNITED_MAIN_THREAD_WAKE_TOKEN)
            .map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("error while setting up main waker: {}", io),
                )
            })
            .bail(9)?,
    );

    let udev = UdevListener::listen(&main_waker).bail(10)?;
    let mod_loaded = mod_loading
        .load_modules(config.sysconf().get_force_modules())
        .bail(11)?;
    setup_vconsole(kcon, &config).bail(12)?;
    let sysfs = SysfsWalker::walk(&main_waker).bail(13)?;

    'main: loop {
        match evloop.poll(
            &mut evs,
            config
                .sysconf()
                .get_mount_timeout()
                .map(Duration::from_secs),
        ) {
            Ok(()) => {}
            Err(io) if io.kind() == ErrorKind::Interrupted => continue,
            Err(io) => Err(io)
                .map_err(|io| {
                    printable_error(
                        PROGRAM_NAME,
                        format!("error while running main event loop: {}", io),
                    )
                })
                .bail(14)?,
        }

        for ev in evs.iter() {
            if ev.token() == IGNITED_MAIN_THREAD_WAKE_TOKEN {
                break 'main;
            }
        }
    }

    udev.stop(kcon);
    sysfs.stop(kcon);

    mod_loaded.wait();

    // TODO: chroot & pivot, cleanup, timer, ...
    let _ = aliases;

    execv(args.init(), &[args.init()])
        .printable(PROGRAM_NAME, INIT_ERROR)
        .bail(101)?;

    // SAFETY: we either shifted execution to init or bailed already.
    unsafe { unreachable_unchecked() }
}
