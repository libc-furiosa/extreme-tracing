#[chrometracer::instrument]
fn bar() {
    println!("hello");
}

#[chrometracer::instrument]
fn foo() {
    bar();
    bar();
}

fn main() {
    let _guard = chrometracer::builder().init();
    foo();
}