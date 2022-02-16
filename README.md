# ignited

Rapid early-ramdisk system initialization, accompanying initramfs generator. **CURRENTLY IN DEVELOPMENT**

## What?
ignited is a simple program meant to run before your Linux system partition has even been
mounted! It is intended to run inside your initramfs and bring your system to a usable
state, at which point it hands off execution to your `/sbin/init` program (which could be
systemd, initd, or any other init program). More specifically, ignited would be your
initramfs' `/init`.

ignited is also the program used to generate initramfs images that contains the ignited
`/init` and necessary kernel modules.

Although written in Rust, ignited is based on [Booster](https://github.com/anatol/booster)
which itself is written in Go. Booster is licensed under
[MIT](https://raw.githubusercontent.com/anatol/booster/master/LICENSE).

### What is an initramfs?
From Booster's README:

    Initramfs is a specially crafted small root filesystem that
    [is] mounted at the early stages of Linux OS boot process.
    This initramfs, among other things, is responsible for [...]
    mounting [the] root filesystem.

In other words, the initramfs is in charge of bringing the system to a usable state,
mounting the root filesystem, and handing off further work to your system's init.

## Where?
ignited only supports Linux systems, and there are no plans to expand compatibility to
other OSes.

If any Linux distribution maintainer/developer is exploring the use of ignited inside
their initramfs archives (or is already doing so), please contact me to be included in
this README. I am also able to give recommendations on proper integration in a distribution
if you contact me.

## Why?
initramfs generation programs generally fall into 2 categories:

- Distro-specific programs such as `mkinitcpio`.
- Universal, extensible programs such as `dracut`.

The former is generally tied to a specific Linux distribution (such as Arch Linux) and
will not function properly outside of it without modification. The latter eschews specific
behaviors in favor of widely-used, mostly distro-agnostic components (such as systemd or
busybox). In most cases, neither are specifically designed for boot time performance nor
image-generation performance. Neither do they strive to be self-contained.

ignited seeks to fulfill three goals:

- Fast image-generation and boot time.
- Compatibility with as many distributions as possible (including ones without systemd,
  busybox, or even udevd).
- Contain as close to no external dependencies as possible.

At the time of this writing, not all goals have been fulfilled. See `GOALS.md` in the
git repo for what progress needs to be made.

### Why not dracut?
- `dracut` doesn't optimize for image-generation speed, as evidenced by
  [this post by Michael Stapelberg](https://michael.stapelberg.ch/posts/2020-01-21-initramfs-from-scratch-golang/)
  in which he recorded ~31s for dracut image generation with `gzip` (lowered to ~9s with
  `pigz`).
- While `dracut` is compatible with many distributions (such as Fedora/RHEL, Debian,
  Arch Linux, openSUSE, among others), its upstream default hooks rely on systemd and/or
  busybox. Theoretically, it seems that special hooks could be made for non-standard
  configurations, but they don't seem to be explicitly supported.
- While `systemd` might be statically compiled with musl, such configuration is non-standard
  and requires various patches. `dracut`'s upstream default hooks also bundle many scripts
  to be executed at boot time, thus requiring additional files and interpreters (plus its
  dynamic link and their libraries). Additionally, image-generation consists on running
  various scripts and binaries to build the image, which may pull in other dependencies.

### Why not mkinitcpio?
- Image-generation takes between 10-20 seconds on my AMD Threadripper 3960X with `mkinitcpio`.
- `mkinitcpio` is specifically designed for Arch Linux and derivatives.
- Two options are given for initramfs generation by default: `udev+busybox`, or `systemd`,
  both of which come with the same problems as described in `Why not dracut?`. Additionally,
  `mkinitcpio` makes use of hooks (similarly to `dracut`), therefore inheriting its drawbacks.

### Why not booster?
ignited is based on `booster`, therefore inheriting its benefits over other initramfs
generators. As time progresses, ignited will differentiate itself from `booster` with specific
features such as verified root, A/B system partitions, and `/vendor` partition support.

## How?
While all code should be properly documented, `docs.rs` currently doesn't support automatic
rustdoc generation for binaries. Work is being done to remedy this and provide proper generated
documentation. In the meantime you can browse the source code.

`ignited(1)` and `ignited(8)` man pages will be made in the future describing the initramfs'
`/init` behavior and the initramfs generator respectively.

## initd?
initd is a simple, serviced-compatible init implementation that is perfectly suitable for
systems with ignited-generated initramfs. ignited can handoff further execution to initd after
the root filesystem has been mounted. You can learn more on
[initd's git repo](https://github.com/hugo-cuenca/initd). Note that having initd installed is
not necessary to use ignited, as it can function just as well if your system uses systemd,
runit, dinit, OpenRC, or whatever init program you choose to use.

License: MITNFA
