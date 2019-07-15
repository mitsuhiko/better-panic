[![Crates.io](https://img.shields.io/crates/v/better-panic.svg)](https://crates.io/crates/better-panic)

# better-panic

`better-panic` gives you pretty backtraces for panics.

It is inspired by Python tracebacks and tries to replicate them as well
as possible.  This is what it looks like:

<img src="https://github.com/mitsuhiko/better-panic/raw/master/screenshot.png">

Some of the code is based on the
[color-backtrace](https://crates.io/crates/color-backtrace) library.

### Usage

The most common way to use it is to invoke the `install` function
which installs a panic handler.  In debug builds the backtrace is shown
automatically, in release builds it's hidden by default.

```rust
better_panic::install();
```

For more configuration see the `Settings` object.

### Features

- Colorize backtraces to be easier on the eyes
- Show source snippets if source files are found on disk
- Hide all the frames after the panic was already initiated

License: MIT OR Apache-2.0
