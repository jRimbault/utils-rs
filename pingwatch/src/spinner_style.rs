include!(concat!(env!("OUT_DIR"), "/spinner_style.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn exposes_all_upstream_spinner_names() {
        #[cfg(feature = "animated-spinners")]
        assert_eq!(SpinnerStyle::value_variants().len(), 90);

        #[cfg(not(feature = "animated-spinners"))]
        assert_eq!(SpinnerStyle::value_variants().len(), 1);
    }

    #[test]
    fn exposes_expected_default_frames() {
        #[cfg(feature = "animated-spinners")]
        assert_eq!(SpinnerStyle::Dots12.frames()[0], "⢀⠀");

        #[cfg(feature = "animated-spinners")]
        assert_eq!(
            SpinnerStyle::SimpleDots.frames(),
            &[".  ", ".. ", "...", "   "]
        );

        #[cfg(not(feature = "animated-spinners"))]
        assert_eq!(SpinnerStyle::default().frames(), &["●", "●"]);
    }
}
