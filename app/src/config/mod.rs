extern crate toml;

use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::Mutex;

#[derive(Debug, Deserialize, Clone)]
pub struct DomainConfig {
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
    pub slack: SlackConfig,
    pub domains: HashMap<String, DomainConfig>,
}

pub struct ConfigState(Mutex<Config>);

impl ConfigState {
    // TODO get rid of expect/unwrap? But crashing here is fine.
    pub fn from_evn() -> Result<ConfigState, failure::Error> {
        let hostname = env::var("HOSTNAME")?;
        let output_dir = env::var("UDRB_OUTPUT")?;

        let config_path = env::var("UDRB_CONFIG")?;
        let config = std::fs::read_to_string(config_path)?;
        let config: toml::Value = config.parse::<toml::Value>()?;

        let mut domains = HashMap::new();
        for (_id, table) in config.get("domain").unwrap().as_table().unwrap() {
            let hosts = table.get("hosts").unwrap().as_array().unwrap();
            let domain_config: DomainConfig = table.clone().try_into().unwrap();
            for host in hosts {
                let host = host.as_str().unwrap().to_string();
                let ret = domains.insert(host.clone(), domain_config.clone());
                ensure!(ret.is_none(), "Duplicate config for domain {}", host)
            }
        }

        let slack = config.get("slack").unwrap();
        let slack_config: SlackConfig = slack.clone().try_into().unwrap();

        Ok(ConfigState(Mutex::new(Config {
            hostname: hostname,
            output_dir: std::path::PathBuf::from_str(&output_dir)?,
            slack: slack_config,
            domains: domains,
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
