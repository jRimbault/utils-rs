include!(concat!(env!("OUT_DIR"), "/spinner_style.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn exposes_all_upstream_spinner_names() {
        assert_eq!(SpinnerStyle::value_variants().len(), 90);
    }

    #[test]
    fn keeps_multi_cell_frames_intact() {
        assert_eq!(SpinnerStyle::Dots12.frames()[0], "⢀⠀");
        assert_eq!(
            SpinnerStyle::SimpleDots.frames(),
            &[".  ", ".. ", "...", "   "]
        );
    }
}
