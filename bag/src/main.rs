use bag::bag::Bag;
use bag::counter::Counter;

fn reverse<T, U>((a, b): (T, U)) -> (U, T) {
    (b, a)
}

fn main() {
    let bag: Counter<_> = "aaaaabbbbbffc".chars().collect();
    println!("{:#?}", bag);
    let bag: Bag<_, _> = "aaaaabbbbbffc".char_indices().map(reverse).collect();
    println!("{:#?}", bag);
}
