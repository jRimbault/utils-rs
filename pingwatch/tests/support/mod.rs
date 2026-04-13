use pingwatch::cli::Args;
use std::{
    ffi::OsString,
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
};

pub const BIN_NAME: &str = "pingwatch";

#[derive(Default)]
pub struct FixtureConfig<'a> {
    pub config_toml: Option<&'a str>,
}

/// Test harness for pingwatch integration tests.
///
/// Every fixture owns a dedicated temp XDG config home and restores the
/// original environment on drop so tests do not leak config state into one
/// another.
pub struct IntegrationFixture {
    _env_lock: MutexGuard<'static, ()>,
    old_xdg_config_home: Option<OsString>,
    config_home: tempfile::TempDir,
}

impl IntegrationFixture {
    pub fn new() -> Self {
        Self::from_config(FixtureConfig::default())
    }

    pub fn with_config(config_toml: &str) -> Self {
        Self::from_config(FixtureConfig {
            config_toml: Some(config_toml),
        })
    }

    pub fn from_config(config: FixtureConfig<'_>) -> Self {
        let env_lock = env_lock().lock().expect("env lock poisoned");
        let config_home = tempfile::tempdir().expect("temp config home");
        let old_xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", config_home.path()) };

        let fixture = Self {
            _env_lock: env_lock,
            old_xdg_config_home,
            config_home,
        };

        if let Some(config_toml) = config.config_toml {
            fixture.write_config(config_toml);
        }

        fixture
    }

    pub fn config_home(&self) -> &Path {
        self.config_home.path()
    }

    pub fn write_config(&self, content: &str) {
        let dir = self.config_home().join(BIN_NAME);
        std::fs::create_dir_all(&dir).expect("create config dir");
        std::fs::write(dir.join("config.toml"), content).expect("write config");
    }

    pub fn parse<I, T>(&self, argv: I) -> anyhow::Result<Args>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Args::parse_from(BIN_NAME, argv)
    }

    #[allow(dead_code)]
    pub async fn run<I, T>(&self, argv: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        pingwatch::run(self.parse(argv)?).await
    }
}

impl Drop for IntegrationFixture {
    fn drop(&mut self) {
        match &self.old_xdg_config_home {
            Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
