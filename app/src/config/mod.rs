use std::str::FromStr;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DomainConfig {
    pub name: String,
    #[serde(with = "serde_regex")]
    pub host: regex::Regex,
    pub login_page: Option<String>,
    // TODO: Wrap in SecretString to hide from debug.
    pub login_script: Option<String>,
    pub render_script: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SlackConfig {
    // If empty, requests are not authenticated.
    // TODO: Consider crashing if empty in production build...
    // TODO: Wrap in SecretString to hide from debug.
    pub secret: Option<String>,
    pub max_age: chrono::TimeDelta,
}

// TODO: Maybe Arc would be better than cloning.
#[derive(Clone, Debug)]
pub struct Config {
    pub hostname: String,
    pub output_dir: std::path::PathBuf,
    pub chrome_address: String,
    pub slack: SlackConfig,
    pub domains: Vec<DomainConfig>,
}

// Helper to include the variable name in the error message.
fn get_env_var(name: &str) -> anyhow::Result<String> {
    std::env::var(name).map_err(|e| anyhow::anyhow!("{}: {}", name, e))
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let hostname = get_env_var("UDRB_HOSTNAME")?;
        let output_dir = get_env_var("UDRB_OUTPUT_DIR")?;
        let chrome_address = get_env_var("UDRB_CHROME_ADDRESS")?;

        let slack = SlackConfig {
            secret: get_env_var("UDRB_SLACK_SECRET").ok(),
            max_age: get_env_var("UDRB_SLACK_MAX_AGE_SECONDS")
                .as_deref()
                .ok()
                .or(Some("120"))
                .map(str::parse::<i64>)
                .and_then(Result::ok)
                .and_then(chrono::TimeDelta::try_seconds)
                .expect("Config max_age_seconds is invalid"),
        };

        let domain_config_path = get_env_var("UDRB_DOMAIN_CONFIG")?;
        let domain_config = std::fs::read_to_string(domain_config_path)?;
        let domains: Vec<DomainConfig> = serde_yaml::from_str(&domain_config)?;

        Ok(Config {
            hostname,
            output_dir: std::path::PathBuf::from_str(&output_dir)?,
            chrome_address,
            slack,
            domains,
        })
    }
}
