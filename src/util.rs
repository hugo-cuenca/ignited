//! Miscellaneous functions that don't fit in any other (rust code) module.

use crate::{early_logging::KConsole, IGNITED_CONFIG, IGNITED_TARGET_ROOT_PATH, PROGRAM_NAME};
use cstr::cstr;
use nix::{
    errno::Errno,
    libc::{dev_t, mode_t, stat as FileStat, S_IFDIR, S_IFMT},
    sys::{
        memfd::{memfd_create, MemFdCreateFlag},
        stat::{lstat as lstat_fn, stat as stat_fn, Mode},
        statfs::{self as StatFs, statfs, FsType as StatFsType},
        utsname::uname
    },
    unistd::{execv, mkdir},
};
use precisej_printable_errno::{ErrnoResult, printable_error, PrintableErrno};
use std::{
    convert::Infallible,
    fs::{read_dir, remove_dir, remove_file, File},
    ffi::{CStr, OsStr, OsString},
    os::unix::{ffi::OsStrExt, io::FromRawFd},
    path::{Path, PathBuf},
    process::id as getpid,
};

/// Remove ramfs without touching the target root.
///
/// We check that the current process is PID1 and that the current initramfs root
/// is either a `ramfs` or a `tmpfs` with the appropriate initramfs files present.
pub fn delete_ramfs() -> Result<(), PrintableErrno<String>> {
    // see statfs(2)
    const RAMFS_MAGIC: StatFsType = StatFsType(0x858458f6);

    fn is_dir(mode: mode_t) -> bool {
        mode & S_IFMT == S_IFDIR
    }
    fn delete_recursive(path: &Path, root_dev: dev_t) -> Result<(), PrintableErrno<String>> {
        let path_stat: FileStat = lstat_fn(path).printable(
            PROGRAM_NAME,
            format!("unable to stat {}", path.display())
        )?;
        if path_stat.st_dev != root_dev {
            // is outside the root initramfs, conserve
            return Ok(())
        }

        if is_dir(path_stat.st_mode) {
            let path_dir_entries = read_dir(path).map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("unable to read {}: {}", path.display(), io),
                )
            })?;

            for entry in path_dir_entries.flatten() {
                if entry.file_name() == "." || entry.file_name() == ".." {
                    delete_recursive(&entry.path(), root_dev)?;
                }
            }
            if path != Path::new("/") {
                // delete directory
                remove_dir(path).map_err(|io| {
                    printable_error(
                        PROGRAM_NAME,
                        format!("unable to remove directory {}: {}", path.display(), io),
                    )
                })?;
            }
        } else if path != Path::new("/") {
            remove_file(path).map_err(|io| {
                printable_error(
                    PROGRAM_NAME,
                    format!("unable to remove file {}: {}", path.display(), io),
                )
            })?;
        }

        Ok(())
    }
    fn exists_in_root(path: &Path, root_dev: dev_t) -> Result<(), PrintableErrno<String>> {
        let path_stat: FileStat = stat_fn(path).printable(
            PROGRAM_NAME,
            format!("unable to stat {}", path.display())
        )?;
        if path_stat.st_dev != root_dev {
            return Err(printable_error(
                PROGRAM_NAME,
                format!("{} is not in our current initramfs", path.display()),
            ));
        }

        Ok(())
    }
    fn full_sanity_check() -> Result<dev_t, PrintableErrno<String>> {
        (getpid() == 1).then(|| ()).ok_or_else(|| {
            printable_error(
                PROGRAM_NAME,
                "not running in an initrd environment, exiting...",
            )
        })?;

        let root_stat: FileStat = stat_fn("/").printable(
            PROGRAM_NAME,
            "unable to stat /",
        )?;
        let root_dev = root_stat.st_dev;

        let new_root_stat: FileStat = stat_fn(IGNITED_TARGET_ROOT_PATH).printable(
            PROGRAM_NAME,
            format!("unable to stat {}", IGNITED_TARGET_ROOT_PATH),
        )?;
        if new_root_stat.st_dev == root_dev {
            return Err(printable_error(
                PROGRAM_NAME,
                format!("/ and {} belong to the same device", IGNITED_TARGET_ROOT_PATH)
            ));
        }

        exists_in_root(Path::new("/etc/initrd-release"), root_dev)?;
        exists_in_root(Path::new(IGNITED_CONFIG), root_dev)?;
        exists_in_root(Path::new("/init"), root_dev)?;

        let root_statfs = statfs("/").printable(
            PROGRAM_NAME,
            "unable to statfs /"
        )?;
        let root_statfs_type = root_statfs.filesystem_type();
        if root_statfs_type != RAMFS_MAGIC
            && root_statfs_type != StatFs::TMPFS_MAGIC {
            return Err(printable_error(
                PROGRAM_NAME,
                "/ should still be initramfs, but is not of type ramfs/tmpfs".to_string(),
            ));
        }
        Ok(root_dev)
    }

    let root_dev = full_sanity_check()?;
    delete_recursive(Path::new("/"), root_dev)
}

/// Get current kernel version. Corresponds to the `release` field in the `utsname`
/// struct returned by `uname(2)`.
///
/// See `uname(2)` for more information.
pub fn get_booted_kernel_ver() -> String {
    uname().release().to_string()
}

/// Create `systemd-state`: a `memfd` containing the timestamp (both realtime and
/// monotonic) when the `ignited` binary started.
pub fn get_systemd_state() -> Result<File, PrintableErrno<String>> {
    let memfd = memfd_create(cstr!("systemd-state"), MemFdCreateFlag::empty())
        .printable(PROGRAM_NAME, "unable to create systemd-state")?;

    // SAFETY: memfd isn't used anywhere else
    Ok(unsafe { File::from_raw_fd(memfd) })
}

/// Check to see if we are running as the `init` inside the initramfs.
///
/// - Make sure we are PID 1.
/// - Check for the existence of `/etc/initrd-release` (see
/// [INITRD_INTERFACE](https://systemd.io/INITRD_INTERFACE/)).
pub fn initial_sanity_check() -> Result<(), PrintableErrno<String>> {
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

/// Get whether the target init system is systemd-compatible.
///
/// Currently assumes that `/path/to/init` is a symbolic link to `/path/to/lib/systemd`
/// on distributions with systemd, as is standard.
pub fn is_systemd_compatible(init_path: &CStr) -> bool {
    let mut init_path = PathBuf::from(
        OsString::from(OsStr::from_bytes(init_path.to_bytes()))
    );
    
    // Max depth of 10 to prevent DoS
    for _ in 0..10 {
        match init_path.read_link() {
            Ok(new) => init_path = new,
            _ => break,
        }
    }
    if let Ok(new) = init_path.canonicalize() {
        init_path = new
    }
    init_path.ends_with("/systemd")
}

/// Create `/run/initramfs`, which can be used by the booted system to switch back to
/// the initramfs environment on shutdown.
///
/// Per [systemd's INITRD_INTERFACE](https://systemd.io/INITRD_INTERFACE/).
pub fn make_shutdown_pivot_dir() -> Result<(), PrintableErrno<String>> {
    let s_rwxu_rxg_rxo =
        Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IXGRP | Mode::S_IROTH | Mode::S_IXOTH;
    loop {
        match mkdir(Path::new("/run/initramfs"), s_rwxu_rxg_rxo) {
            Ok(()) => break Ok(()),
            Err(e) if e == Errno::ENOENT => {
                // Recurse and try again
            }
            Err(e) => {
                break Err(e).printable(
                    PROGRAM_NAME,
                    "FATAL: unable to create /run/initramfs".to_string(),
                )
            }
        }
    }
}

/// Spawn an emergency shell.
///
/// Currently this function attempts to spawn `/bin/busybox` first. If it doesn't exist,
/// it will attempt `/bin/toybox` instead. If none exists (or all of them fail to
/// properly handover execution), this function logs an error to `kmsg`.
///
/// If the emergency shell is spawned, this function never returns.
pub fn spawn_emergency_shell(kcon: &mut KConsole) -> Result<Infallible, ()> {
    kcrit!(kcon, "attempting to spawn emergency shell");

    let argv = [cstr!("sh"), cstr!("-I")];
    let exists_b = match execv(cstr!("/bin/busybox"), &argv).unwrap_err() {
        Errno::ENOENT => false,
        e => {
            let e = Err::<(), _>(e)
                .printable(PROGRAM_NAME, "unable to execute /bin/busybox")
                .unwrap_err();
            kcrit!(kcon, "{}", e);
            true
        }
    };

    // If we are here, busybox doesn't exist or execv failed, so try toybox
    let err_t = match execv(cstr!("/bin/toybox"), &argv).unwrap_err() {
        Errno::ENOENT => None,
        e => Some(
            Err::<(), _>(e)
                .printable(PROGRAM_NAME, "unable to execute /bin/toybox")
                .unwrap_err(),
        ),
    };
    // Both failed to execute
    if !exists_b {
        kcrit!(
            kcon,
            "unable to execute /bin/busybox: {}",
            Errno::ENOENT.desc()
        );
    }

    match err_t {
        Some(e) => {
            kcrit!(kcon, "{}", e);
        }
        None => {
            kcrit!(
                kcon,
                "unable to execute /bin/toybox: {}",
                Errno::ENOENT.desc()
            );
        }
    }

    Err(())
}
