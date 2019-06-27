fn bar() {
    vec![42u32][10];
}

fn foo() {
    bar();
}

fn main() {
    better_panic::Settings::debug()
        .most_recent_first(false)
        .lineno_suffix(true)
        .install();
    foo();
}
