// extern crate toml;
use std::str::FromStr;

// #[derive(Debug, Clone)]
// pub struct DomainConfig {
//     pub id: String,
//     pub host_regex: regex::Regex,
//     pub login_page: Option<String>,
//     pub login_script: Option<String>,
//     pub render_script: Option<String>,
// }

// #[derive(Debug, Deserialize, Clone)]
// pub struct SlackConfig {
//     pub secret: Option<String>,
//     pub max_age_seconds: Option<i64>,
// }

#[derive(Debug, Clone)]
pub struct Config {
    //     pub hostname: String,
    pub output_dir: std::path::PathBuf,
    //     pub chrome_address: String,
    //     pub slack: SlackConfig,
    //     pub domains: Vec<DomainConfig>,
}

// Helper to include the variable name in the error message.
fn get_env_var(name: &str) -> anyhow::Result<String> {
    std::env::var(name).map_err(|e| anyhow::anyhow!("{}: {}", name, e))
}

impl Config {
    //     // TODO get rid of expect/unwrap? But crashing here is fine.
    pub fn from_env() -> anyhow::Result<Config> {
        //         let hostname = env::var("HOSTNAME")?;
        let output_dir = get_env_var("UDRB_OUTPUT")?;
        //         let chrome_address = env::var("UDRB_CHROME_ADDRESS")?;

        //         let config_path = env::var("UDRB_CONFIG")?;
        //         let config = std::fs::read_to_string(config_path)?;
        //         let config: toml::Value = config.parse::<toml::Value>()?;

        //         let mut domains = Vec::new();
        //         for (id, table) in config.get("domain").unwrap().as_table().unwrap() {
        //             let host_regex = regex::RegexBuilder::new(table.get("host").unwrap().as_str().unwrap())
        //                 .case_insensitive(true)
        //                 .build()
        //                 .unwrap();
        //             domains.push(DomainConfig {
        //                 id: id.to_string(),
        //                 host_regex,
        //                 login_page: table
        //                     .get("login_page")
        //                     .map(|x| x.as_str().unwrap().to_string()),
        //                 login_script: table
        //                     .get("login_script")
        //                     .map(|x| x.as_str().unwrap().to_string()),
        //                 render_script: table
        //                     .get("render_script")
        //                     .map(|x| x.as_str().unwrap().to_string()),
        //             });
        //         }
        //         let slack = config.get("slack").unwrap();

        Ok(Config {
            //             hostname,
            output_dir: std::path::PathBuf::from_str(&output_dir)?,
            //             chrome_address,
            //             slack: slack.clone().try_into().unwrap(),
            //             domains,
        })
    }

    //     pub fn get(&self) -> Config {
    //         // TODO avoid this pointless copy...
    //         self.0.lock().unwrap().clone()
    //     }

    //     pub fn get_slack(&self) -> SlackConfig {
    //         // TODO avoid this pointless copy...
    //         self.0.lock().unwrap().slack.clone()
    //     }
}
