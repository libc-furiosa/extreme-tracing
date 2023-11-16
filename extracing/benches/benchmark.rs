use criterion::{criterion_group, criterion_main, Criterion};
use extracing::event;
use chrometracer::{builder, event as cevent};

//#[chrometracer::instrument(fields(namee = format!("{}", "bye"), tid = 1))]
#[chrometracer::instrument(fields(namee = format!("{}", "bye"), tid = 1), skip(a))]
fn hello() {
    //println!("HELLO WORLD");
}

fn extreme_skip(c: &mut Criterion) {
    c.bench_function("span_noop", |b| {
        b.iter(|| {
            event!(name="hello");
        })
    });
}

fn extreme_span(c: &mut Criterion) {
    let _guard = extracing::tracer::Tracer::init();
    c.bench_function("span", |b| {
        b.iter(|| {
            event!(name=hello,i=1, f=3.4);
        })
    });
}

fn chrometracer(c: &mut Criterion) {
    let _guard = chrometracer::builder().init();
    c.bench_function("event", |b| {
        
        b.iter(|| {
            cevent!(name: "hello", from: std::time::Duration::from_secs(1), to: std::time::Duration::from_secs(2), is_async: false);
        })
    });
}

fn instrument(c: &mut Criterion) {
    let _guard = chrometracer::builder().init();
    c.bench_function("instrument", |b| {
        b.iter(|| {
            hello();
        })
    });
}

fn get_thread_id(c: &mut Criterion) {
    c.bench_function("tid", |b| {
        b.iter(|| {
            let _t = std::thread::current().id();
        })
    });
}

criterion_group!(
    benches,

    //extreme_skip,
    extreme_span,
    chrometracer,
    instrument,
    get_thread_id,
);
criterion_main!(benches);