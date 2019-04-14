extern crate toml;

use std::env;
use std::str::FromStr;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct DomainConfig {
    pub id: String,
    pub host_regex: regex::Regex,
    pub login_page: Option<String>,
    pub login_script: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SlackConfig {
    pub secret: Option<String>,
    pub max_age_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub hostname: String,
    pub output_dir: std::path::PathBuf,
    pub chrome_address: String,
    pub slack: SlackConfig,
    pub domains: Vec<DomainConfig>,
}

pub struct ConfigState(Mutex<Config>);

impl ConfigState {
    // TODO get rid of expect/unwrap? But crashing here is fine.
    pub fn from_evn() -> Result<ConfigState, failure::Error> {
        let hostname = env::var("HOSTNAME")?;
        let output_dir = env::var("UDRB_OUTPUT")?;
        let chrome_address = env::var("UDRB_CHROME_ADDRESS")?;

        let config_path = env::var("UDRB_CONFIG")?;
        let config = std::fs::read_to_string(config_path)?;
        let config: toml::Value = config.parse::<toml::Value>()?;

        let mut domains = Vec::new();
        for (id, table) in config.get("domain").unwrap().as_table().unwrap() {
            let host_regex = regex::RegexBuilder::new(table.get("host").unwrap().as_str().unwrap())
                .case_insensitive(true)
                .build()
                .unwrap();
            domains.push(DomainConfig {
                id: id.to_string(),
                host_regex,
                login_page: table
                    .get("login_page")
                    .map(|x| x.as_str().unwrap().to_string()),
                login_script: table
                    .get("login_script")
                    .map(|x| x.as_str().unwrap().to_string()),
            });
        }
        let slack = config.get("slack").unwrap();

        Ok(ConfigState(Mutex::new(Config {
            hostname,
            output_dir: std::path::PathBuf::from_str(&output_dir)?,
            chrome_address,
            slack: slack.clone().try_into().unwrap(),
            domains,
        })))
    }

    pub fn get(&self) -> Config {
        // TODO avoid this pointless copy...
        self.0.lock().unwrap().clone()
    }

    pub fn get_slack(&self) -> SlackConfig {
        // TODO avoid this pointless copy...
        self.0.lock().unwrap().slack.clone()
    }
}
