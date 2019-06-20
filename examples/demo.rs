use rand::{thread_rng, Rng};

fn bar() {
    vec![42u32][10];
    let mut rng = thread_rng();
    let _n: u32 = rng.gen_range(100, 10);
}

fn foo() {
    bar();
}

fn main() {
    better_panic::Settings::debug()
        .most_recent_first(false)
        .install();
    foo();
}
