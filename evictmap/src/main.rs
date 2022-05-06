use evictmap::EvictMap;

fn main() {
    let mut map = EvictMap::default();
    println!("{}", map.add("apibox"));
    println!("{}", map.add("apibox"));
    println!("{}", map.add("apibox"));
    println!("{map:#?}");
    map.remove("apibox", 1);
    map.remove("apibox", 0);
    println!("{map:#?}");
    println!("{}", map.add("apibox"));
    println!("{}", map.add("apibox"));
    println!("{}", map.add("sitebox"));
    println!("{map:#?}");
}
