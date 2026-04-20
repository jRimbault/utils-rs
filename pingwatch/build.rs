use indexmap::IndexMap;
use serde::Deserialize;
use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const SOURCE_CONFIG_PATH: &str = "spinner-source.toml";
const LOCAL_SOURCE_PATH: &str = "src/spinners.json";
const GENERATED_MODULE_PATH: &str = "spinner_style.rs";
const REFRESH_ENV: &str = "PINGWATCH_REFRESH_SPINNERS";
const ANIMATED_FEATURE_ENV: &str = "CARGO_FEATURE_ANIMATED_SPINNERS";

#[derive(Deserialize)]
struct SpinnerSourceConfig {
    upstream: UpstreamSource,
}

#[derive(Deserialize)]
struct UpstreamSource {
    owner: String,
    repo: String,
    revision: String,
    path: String,
}

impl UpstreamSource {
    fn url(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.owner, self.repo, self.revision, self.path
        )
    }
}

#[derive(Deserialize)]
struct SpinnerSpec {
    interval: u64,
    frames: Vec<String>,
}

fn main() {
    println!("cargo:rerun-if-changed={SOURCE_CONFIG_PATH}");
    println!("cargo:rerun-if-changed={LOCAL_SOURCE_PATH}");
    println!("cargo:rerun-if-env-changed={REFRESH_ENV}");
    println!("cargo:rerun-if-env-changed={ANIMATED_FEATURE_ENV}");

    let out_path = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is always set"))
        .join(GENERATED_MODULE_PATH);
    if env::var_os(ANIMATED_FEATURE_ENV).is_none() {
        let generated = generate_static_module();
        write_if_changed(&out_path, generated.as_bytes())
            .unwrap_or_else(|err| panic!("writing {}: {err}", out_path.display()));
        return;
    }

    let config = read_source_config();
    let local_source_path = Path::new(LOCAL_SOURCE_PATH);

    if env::var_os(REFRESH_ENV).is_some() || !local_source_path.exists() {
        match download_upstream_source(&config.upstream) {
            Ok(body) => write_if_changed(local_source_path, body.as_bytes())
                .unwrap_or_else(|err| panic!("writing {LOCAL_SOURCE_PATH}: {err}")),
            Err(err) if local_source_path.exists() => {
                println!(
                    "cargo:warning=failed to refresh {LOCAL_SOURCE_PATH} from pinned upstream: {err}"
                );
            }
            Err(err) => {
                panic!(
                    "downloading {LOCAL_SOURCE_PATH} from pinned upstream {}: {err}",
                    config.upstream.url()
                );
            }
        }
    }

    let source = fs::read_to_string(local_source_path)
        .unwrap_or_else(|err| panic!("reading {LOCAL_SOURCE_PATH}: {err}"));
    let spinners: IndexMap<String, SpinnerSpec> = serde_json::from_str(&source)
        .unwrap_or_else(|err| panic!("parsing {LOCAL_SOURCE_PATH}: {err}"));

    let generated = generate_module(&config.upstream, &spinners);
    write_if_changed(&out_path, generated.as_bytes())
        .unwrap_or_else(|err| panic!("writing {}: {err}", out_path.display()));
}

fn generate_static_module() -> String {
    let mut output = String::new();

    writeln!(&mut output, "use clap::ValueEnum;").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "/// Static dot renderer used when the `animated-spinners` feature is disabled."
    )
    .unwrap();
    writeln!(
        &mut output,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, serde::Deserialize)]"
    )
    .unwrap();
    writeln!(&mut output, "#[value(rename_all = \"camelCase\")]").unwrap();
    writeln!(&mut output, "#[serde(rename_all = \"camelCase\")]").unwrap();
    writeln!(&mut output, "pub enum SpinnerStyle {{").unwrap();
    writeln!(&mut output, "    #[default]").unwrap();
    writeln!(&mut output, "    StaticDot,").unwrap();
    writeln!(&mut output, "}}").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "pub const DEFAULT_SPINNER_STYLE_NAME: &str = \"staticDot\";"
    )
    .unwrap();
    writeln!(
        &mut output,
        "pub const SPINNER_SOURCE_REVISION: &str = \"static-dot\";"
    )
    .unwrap();
    writeln!(
        &mut output,
        "pub const SPINNER_SOURCE_URL: &str = \"static-dot\";"
    )
    .unwrap();
    writeln!(&mut output).unwrap();
    writeln!(&mut output, "impl SpinnerStyle {{").unwrap();
    writeln!(&mut output, "    pub const fn interval_ms(self) -> u64 {{").unwrap();
    writeln!(&mut output, "        match self {{").unwrap();
    writeln!(&mut output, "            Self::StaticDot => 1000,").unwrap();
    writeln!(&mut output, "        }}").unwrap();
    writeln!(&mut output, "    }}").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "    pub const fn frames(self) -> &'static [&'static str] {{"
    )
    .unwrap();
    writeln!(&mut output, "        match self {{").unwrap();
    writeln!(
        &mut output,
        "            Self::StaticDot => &[\"●\", \"●\"],"
    )
    .unwrap();
    writeln!(&mut output, "        }}").unwrap();
    writeln!(&mut output, "    }}").unwrap();
    writeln!(&mut output, "}}").unwrap();

    output
}

fn read_source_config() -> SpinnerSourceConfig {
    let content = fs::read_to_string(SOURCE_CONFIG_PATH)
        .unwrap_or_else(|err| panic!("reading {SOURCE_CONFIG_PATH}: {err}"));
    toml::from_str(&content).unwrap_or_else(|err| panic!("parsing {SOURCE_CONFIG_PATH}: {err}"))
}

fn download_upstream_source(source: &UpstreamSource) -> Result<String, Box<dyn std::error::Error>> {
    let response = reqwest::blocking::Client::builder()
        .build()?
        .get(source.url())
        .header(reqwest::header::USER_AGENT, "pingwatch/build.rs")
        .send()?
        .error_for_status()?;
    Ok(response.text()?)
}

fn generate_module(source: &UpstreamSource, spinners: &IndexMap<String, SpinnerSpec>) -> String {
    let mut output = String::new();

    writeln!(&mut output, "use clap::ValueEnum;").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "/// Spinner presets imported from <https://github.com/{}/{}>.",
        source.owner, source.repo
    )
    .unwrap();
    writeln!(
        &mut output,
        "/// Generated from `{}` at revision `{}` by `build.rs`.",
        source.path, source.revision
    )
    .unwrap();
    writeln!(
        &mut output,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, serde::Deserialize)]"
    )
    .unwrap();
    writeln!(&mut output, "#[value(rename_all = \"camelCase\")]").unwrap();
    writeln!(&mut output, "#[serde(rename_all = \"camelCase\")]").unwrap();
    writeln!(&mut output, "pub enum SpinnerStyle {{").unwrap();
    for name in spinners.keys() {
        if name == "dots14" {
            writeln!(&mut output, "    #[default]").unwrap();
        }
        writeln!(&mut output, "    {},", rust_variant(name)).unwrap();
    }
    writeln!(&mut output, "}}").unwrap();
    writeln!(&mut output).unwrap();

    writeln!(
        &mut output,
        "pub const SPINNER_SOURCE_REVISION: &str = {:?};",
        source.revision
    )
    .unwrap();
    writeln!(
        &mut output,
        "pub const DEFAULT_SPINNER_STYLE_NAME: &str = \"dots14\";"
    )
    .unwrap();
    writeln!(
        &mut output,
        "pub const SPINNER_SOURCE_URL: &str = {:?};",
        source.url()
    )
    .unwrap();
    writeln!(&mut output).unwrap();

    writeln!(&mut output, "impl SpinnerStyle {{").unwrap();
    writeln!(&mut output, "    pub const fn interval_ms(self) -> u64 {{").unwrap();
    writeln!(&mut output, "        match self {{").unwrap();
    for (name, spec) in spinners {
        writeln!(
            &mut output,
            "            Self::{} => {},",
            rust_variant(name),
            spec.interval
        )
        .unwrap();
    }
    writeln!(&mut output, "        }}").unwrap();
    writeln!(&mut output, "    }}").unwrap();
    writeln!(&mut output).unwrap();

    writeln!(
        &mut output,
        "    pub const fn frames(self) -> &'static [&'static str] {{"
    )
    .unwrap();
    writeln!(&mut output, "        match self {{").unwrap();
    for (name, spec) in spinners {
        let frames = spec
            .frames
            .iter()
            .map(|frame| serde_json::to_string(frame).expect("frame is serializable"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            &mut output,
            "            Self::{} => &[{}],",
            rust_variant(name),
            frames
        )
        .unwrap();
    }
    writeln!(&mut output, "        }}").unwrap();
    writeln!(&mut output, "    }}").unwrap();
    writeln!(&mut output, "}}").unwrap();

    output
}

fn rust_variant(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut out = String::new();
    out.extend(first.to_uppercase());
    out.extend(chars);
    out
}

fn write_if_changed(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
