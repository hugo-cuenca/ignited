# ignited

Rapid early-ramdisk system initialization, accompanying initramfs generator. **CURRENTLY IN DEVELOPMENT**

## What?
ignited is a simple program meant to run before your Linux system partition has even been
mounted! It is intended to run inside your initramfs and bring your system to a usable
state, at which point it hands off execution to your `/sbin/init` program.

ignited, although written in Rust, is based on [Booster](https://github.com/anatol/booster)
which is written in Go. Booster is licensed under MIT, a copy of which can be found
[here](https://raw.githubusercontent.com/anatol/booster/master/LICENSE).

### What is an initramfs?
From Booster's README:

    Initramfs is a specially crafted small root filesystem
    that mounted at the early stages of Linux OS boot process.
    This initramfs among other things is responsible for
    [...] mounting [the] root filesystem.

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
## How?
## initd?
To be written.

***

This crate currently serves the purpose of reserving the name `ignited` in crates.io,
and contains no other code than the standard "Hello, world!".

License: MITNFA
