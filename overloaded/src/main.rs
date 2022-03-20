use overloaded::color;

fn main() {
    let _ = dbg!(color::rgb((1.0, 0.004, 0.25)));
    let _ = dbg!(color::rgb("#FF0140"));
    let _ = dbg!(color::rgb((255, 1, 64)));
    let _ = dbg!(color::rgb((255, 1.0, 255)));
    let c = color::rgb("#FF0140").unwrap();
    println!("{c} or {c:X}");
}
