use std::time::Instant;
use std::cell::RefCell;

thread_local! {
    static ITEMS: RefCell<([u64; 8], usize)>  = RefCell::new(([0; 8], 0));
}

pub fn print_item() {
    ITEMS.with(|x| {
        println!("{:?}", x);
    });
}

#[inline]
pub fn add_item(number: u64) {
    let from = Instant::now();
    ITEMS.with(|x| {
        println!("elapsed 1 {}", from.elapsed().as_nanos());
        let c = {
            x.borrow().1
        };
        println!("elapsed 2 {}", from.elapsed().as_nanos());
        
        let mut a = x.borrow_mut();
        a.0[c] = number;
        a.1 = a.1 + 1;
        println!("elapsed 3 {}", from.elapsed().as_nanos());
    });
    println!("elapsed 4 {}", from.elapsed().as_nanos());
}

pub struct Span {
    pub name: &'static str,
    pub from: Option<std::time::Duration>,
    pub to: Option<std::time::Duration>,
}

#[macro_export]
macro_rules! span {
    ($($key:ident = $value:expr),*) => {
        $crate::add_item(1);
    };
}

#[cfg(test)]
mod tests {

    #[test]
    fn ignore_span() {
        span!(name = "hello");
        span!(name = "hello");
        span!(name = "hello");
        span!(name = "hello");
        crate::print_item();
    }

}