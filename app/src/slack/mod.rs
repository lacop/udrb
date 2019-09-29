extern crate chrono;
extern crate hex;
extern crate serde_qs;

use crate::config::{ConfigState, SlackConfig};
use crate::renderer::{RenderError, RenderRequest, RenderResult};

use chrono::{TimeZone, Utc};

use crypto::hmac::Hmac;
use crypto::mac::Mac;
use crypto::sha2::Sha256;

use rocket::data::Data;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::{Outcome, State};

use std::fmt::Write;
use std::io::Read;

#[derive(Debug, Deserialize)]
pub struct SlashRequest {
    command: String,
    text: String,
    response_url: String,
    user_name: Option<String>,
    channel_name: Option<String>,
    team_domain: Option<String>,
}

impl SlashRequest {
    pub fn render_and_reply(self) -> (Option<RenderRequest>, SlackMessage) {
        if self.command != "/udrb" {
            return (
                None,
                SlackMessage::ephemeral("Unknown command. Use /udrb http://...".to_string()),
            );
        }

        let url = url::Url::parse(&self.text).ok();
        if url.is_none() {
            return (
                None,
                SlackMessage::ephemeral("Invalid arguments. Use /udrb http://...".to_string()),
            );
        }

        let request = RenderRequest {
            url: url.unwrap(),
            slack_callback: Some(self.response_url),
            user: self.user_name,
            channel: self.channel_name,
            team: self.team_domain,
        };
        (
            Some(request),
            SlackMessage::ephemeral("Downloading...".to_string()),
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackResponseType {
    Ephemeral,
    InChannel,
}

#[derive(Debug, Serialize)]
pub struct SlackMessage {
    response_type: SlackResponseType,
    text: String,
    #[serde(rename = "mrkdwn")]
    markdown: bool,
}

impl SlackMessage {
    fn ephemeral(text: String) -> SlackMessage {
        SlackMessage {
            response_type: SlackResponseType::Ephemeral,
            text,
            markdown: false,
        }
    }
}

pub struct SlackRequestParser {
    timestamp: i64,
    signature: String,
    config: SlackConfig,
}

#[derive(Debug)]
pub enum SlackParserError {
    ConfigError,
    MissingHeaders,
    BadQueryString,
    TimestampTooOld,
    SignatureInvalid,
}

impl<'a, 'r> FromRequest<'a, 'r> for SlackRequestParser {
    type Error = SlackParserError;

    fn from_request(req: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let config = req.guard::<State<ConfigState>>().succeeded();
        if config.is_none() {
            return Outcome::Failure((Status::InternalServerError, SlackParserError::ConfigError));
        }
        let slack_config = config.unwrap().get_slack();

        let timestamp = req.headers().get_one("X-Slack-Request-Timestamp");
        let timestamp = timestamp.and_then(|t| t.parse::<i64>().ok());
        let signature = req.headers().get_one("X-Slack-Signature");

        match (timestamp, signature) {
            (Some(t), Some(s)) => Outcome::Success(SlackRequestParser {
                timestamp: t,
                signature: s.to_string(),
                config: slack_config,
            }),
            _ => Outcome::Failure((Status::Unauthorized, SlackParserError::MissingHeaders)),
        }
    }
}

impl SlackRequestParser {
    pub fn parse_slash(&self, raw_data: Data) -> Result<SlashRequest, SlackParserError> {
        // TODO size limit here to avoid crashing?
        let mut data = String::new();
        raw_data
            .open()
            .read_to_string(&mut data)
            .map_err(|_| SlackParserError::BadQueryString)?;

        let request = serde_qs::from_str(&data).map_err(|_| SlackParserError::BadQueryString)?;

        // Verify timestamp.
        if self.config.max_age_seconds.is_some() {
            let request_time = Utc.timestamp(self.timestamp, 0);
            let current_time = chrono::Local::now();
            let elapsed = current_time.signed_duration_since(request_time);
            if elapsed > time::Duration::seconds(self.config.max_age_seconds.unwrap()) {
                println!("Rejecting old timestamp: {:?}", elapsed);
                return Err(SlackParserError::TimestampTooOld);
            }
        }

        // Verify signature.
        if self.config.secret.is_some() {
            // Remove the version prefix and parse as hex.
            let signature = self.signature.trim_start_matches("v0=");
            let signature =
                hex::decode(signature).map_err(|_| SlackParserError::SignatureInvalid)?;

            // Concat version, timestamp and data for hmac.
            let basestring = format!("v0:{}:{}", self.timestamp, data);

            let mut hmac = Hmac::new(
                Sha256::new(),
                self.config.secret.as_ref().unwrap().as_bytes(),
            );
            hmac.input(basestring.as_bytes());

            if !crypto::util::fixed_time_eq(hmac.result().code(), &signature) {
                println!("Rejecting bad signature");
                return Err(SlackParserError::SignatureInvalid);
            }
        }

        Ok(request)
    }
}

// From https://api.slack.com/docs/message-formatting
fn slack_encode(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
}

fn post_slack_message(callback: &str, message: SlackMessage) -> Result<(), failure::Error> {
    let client = reqwest::Client::new();
    let response = client.post(callback).json(&message).send()?;
    if !response.status().is_success() {
        return Err(format_err!("Request failed: {:?}", response));
    }
    Ok(())
}

pub fn post_success(callback: &str, result: &RenderResult) -> Result<(), failure::Error> {
    let mut text = String::new();
    if result.user.is_some() {
        writeln!(
            &mut text,
            ":bust_in_silhouette: {}",
            result.user.as_ref().unwrap()
        )
        .unwrap();
    }
    writeln!(
        &mut text,
        ":page_with_curl: *{}*",
        slack_encode(&result.title)
    )
    .unwrap();
    writeln!(&mut text, ":lock: <{}|Original link>", result.orig_url).unwrap();
    write!(&mut text, ":unlock: <{}|PDF version>", result.pdf_url).unwrap();
    if result.png_url.is_some() {
        write!(
            &mut text,
            "\n:camera: <{}|Screenshot>",
            result.png_url.as_ref().unwrap()
        );
    }

    post_slack_message(
        callback,
        SlackMessage {
            response_type: SlackResponseType::InChannel,
            markdown: true,
            text,
        },
    )
}

pub fn post_failure(callback: &str, error: &RenderError) -> Result<(), failure::Error> {
    post_slack_message(
        callback,
        SlackMessage::ephemeral(format!("Error downloading: {}", error)),
    )
}
