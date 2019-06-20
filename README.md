# better-panic

`better-panic` gives you pretty backtraces for panics.

It's originally based on the [color-backtrace](https://crates.io/crates/color-backtrace)
library but it crates backtraces that look more like Python tracebacks.

### Usage

The most common way to use it is to invoke the `install` function
which installs a panic handler.  In debug builds the backtrace is shown
automatically, in release builds it's hidden by default.

```rust
pretty_backtrace::install();
```

For more configuration see the `Settings` object.

### Features

- Colorize backtraces to be easier on the eyes
- Show source snippets if source files are found on disk
- Hide all the frames after the panic was already initiated

License: MIT OR Apache-2.0
