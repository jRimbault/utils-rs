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

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Target::Fragment => "fragment",
            Target::Host => "host",
            Target::Password => "password",
            Target::Path => "path",
            Target::Port => "port",
            Target::Query => "query",
            Target::Scheme => "scheme",
            Target::User => "user",
        };
        write!(f, "{s}")
    }
}
