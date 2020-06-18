# libxm-rs

[![Build Status](https://travis-ci.org/nukep/libxm-rs.svg)](https://travis-ci.org/nukep/libxm-rs)

A binding of [libxm](https://github.com/Artefact2/libxm/) for Rust.

A small XM (FastTracker II Extended Module) player library. Designed
for easy integration in demos and such, and provides timing functions
for easy sync against specific instruments, samples or channels.

As with libxm, this library is released under the WTFPL license.

**Documentation**: https://nukep.github.io/libxm-rs/libxm

## Build requirements

`libxm-rs` is ready to use with Rust 1.0 stable, and should be up to date with
the nightly builds.

If `libxm` is built locally (this is the default!), you must have a C compiler
on your system that supports the C11 standard (such as GCC 4.7+ or clang 3.1).
If you don't wish to build locally, a shared library that you have pre-built
can be provided by following the steps below.

## Linking to a shared version of `libxm`
By default, `libxm-rs` statically links and compiles `libxm`.
This is to allow users to get started with the library more quickly.

If you wish to provide your own shared or custom version of `libxm`, you can
override the build step for `xm` in a `.cargo/config` file
(see http://doc.crates.io/build-script.html#overriding-build-scripts).

```toml
[target.x86_64-unknown-linux-gnu.xm]
rustc-flags = "-l xm"
```
