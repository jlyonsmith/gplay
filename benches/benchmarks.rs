use core::fmt::Arguments;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gplay::*;

fn benchmark_1(arg: &str) {
    struct TestLogger;

    impl TestLogger {
        fn new() -> TestLogger {
            TestLogger {}
        }
    }

    impl GplayLog for TestLogger {
        fn output(self: &Self, _args: Arguments) {}
        fn warning(self: &Self, _args: Arguments) {}
        fn error(self: &Self, _args: Arguments) {}
    }

    let logger = TestLogger::new();
    let mut tool = GplayTool::new(&logger);
    let args: Vec<std::ffi::OsString> = vec!["".into(), arg.into()];

    tool.run(args).unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("basic test", |b| {
        b.iter(|| benchmark_1(black_box("--help")))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
