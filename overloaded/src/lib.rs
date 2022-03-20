pub mod color;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn build_same_color_with_ints_or_string() {
        assert_eq!(color::rgb("#FF0040"), color::rgb((255, 0, 64)));
        let c = color::rgb("#FF0040").unwrap();
        let c = format!("{:X}", c);
        assert_eq!(c, "#FF0040");
    }
}
