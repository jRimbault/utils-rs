use clap::ValueEnum;

use crate::{SetAction, Target};

impl std::str::FromStr for SetAction {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((target, value)) = s.split_once('=') {
            match <Target as ValueEnum>::from_str(target, true) {
                Ok(target) => Ok(SetAction {
                    target,
                    value: value.to_owned(),
                }),
                Err(error) => Err(error),
            }
        } else {
            Err(format!(r#"should be "url_part=value" got {s:?}"#))
        }
    }
}
