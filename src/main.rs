//! Rapid early-ramdisk system initialization, accompanying initramfs generator. **CURRENTLY IN DEVELOPMENT**
//!
//! # What?
//! ignited is a simple program meant to run before your Linux system partition has even been
//! mounted! This crate currently serves the purpose of reserving the name `ignited` in crates.io,
//! and contains no other code than the standard "Hello, world!".
#![crate_name = "ignited"]
#![cfg_attr(test, deny(warnings))]
#![deny(unused)]
#![deny(unstable_features)]
#![warn(missing_docs)]
#![allow(rustdoc::private_intra_doc_links)]

/// Entry-point for ignited.
fn main() {
    println!("Hello, world!");
}
