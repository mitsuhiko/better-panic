# better-panic

[![Build Status](https://github.com/mitsuhiko/better-panic/workflows/Tests/badge.svg?branch=master)](https://github.com/mitsuhiko/better-panic/actions?query=workflow%3ATests)
[![Crates.io](https://img.shields.io/crates/d/better-panic.svg)](https://crates.io/crates/better-panic)
[![License](https://img.shields.io/github/license/mitsuhiko/better-panic)](https://github.com/mitsuhiko/better-panic/blob/master/LICENSE)
[![rustc 1.42.0](https://img.shields.io/badge/rust-1.42%2B-orange.svg)](https://img.shields.io/badge/rust-1.42%2B-orange.svg)
[![Documentation](https://docs.rs/better-panic/badge.svg)](https://docs.rs/better-panic)

`better-panic` gives you pretty backtraces for panics.

It is inspired by Python tracebacks and tries to replicate them as well
as possible.  This is what it looks like:

<img src="https://github.com/mitsuhiko/better-panic/raw/master/screenshot.png">

Some of the code is based on the
[color-backtrace](https://crates.io/crates/color-backtrace) library.

## Usage

The most common way to use it is to invoke the `install` function
which installs a panic handler.  In debug builds the backtrace is shown
automatically, in release builds it's hidden by default.

```rust
better_panic::install();
```

For more configuration see the `Settings` object.

## Features

- Colorize backtraces to be easier on the eyes
- Show source snippets if source files are found on disk
- Hide all the frames after the panic was already initiated

## License and Links

- [Documentation](https://docs.rs/better-panic/)
- [Issue Tracker](https://github.com/mitsuhiko/better-panic/issues)
- [Examples](https://github.com/mitsuhiko/better-panic/tree/master/examples)
- License: [MIT](https://github.com/mitsuhiko/better-panic/blob/master/LICENSE)
