// extern crate chrono;
// extern crate hex;
// extern crate serde_qs;

// use crate::config::{ConfigState, SlackConfig};
// use crate::renderer::{RenderError, RenderRequest, RenderResult};
use crate::renderer::RenderRequest;
use chrono::{TimeZone, Utc};

// use crypto::hmac::Hmac;
// use crypto::mac::Mac;
// use crypto::sha2::Sha256;

use crate::config::{Config, SlackConfig};
use log::error;

use rocket::data::{Data, ToByteUnit};
use rocket::http::Status;
use rocket::request::Outcome;
use rocket::request::{self, FromRequest, Request};
use rocket::State;

// use std::fmt::Write;
// use std::io::Read;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SlashRequest {
    command: String,
    text: String,
    response_url: String,
    // TODO: Deprecated, use user_id and channel_id instead.
    user_name: Option<String>,
    channel_name: Option<String>,
    // TODO: Not actually set?
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

        let url = match url::Url::parse(&self.text) {
            Ok(url) => url,
            Err(_) => {
                return (
                    None,
                    SlackMessage::ephemeral("Invalid arguments. Use /udrb http://...".to_string()),
                );
            }
        };

        (
            Some(RenderRequest {
                url,
                slack_callback: self.response_url,
                user: self.user_name,
                channel: self.channel_name,
                team: self.team_domain,
            }),
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
    TimestampTooDifferent,
    SignatureInvalid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SlackRequestParser {
    type Error = SlackParserError;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let config = req.guard::<&State<Config>>().await;
        let slack_config = if let Outcome::Success(config) = config {
            config.slack.clone()
        } else {
            return Outcome::Error((Status::InternalServerError, SlackParserError::ConfigError));
        };

        let timestamp = req
            .headers()
            .get_one("X-Slack-Request-Timestamp")
            .and_then(|t| t.parse::<i64>().ok());
        let signature = req.headers().get_one("X-Slack-Signature");

        match (timestamp, signature) {
            (Some(t), Some(s)) => Outcome::Success(SlackRequestParser {
                timestamp: t,
                signature: s.to_string(),
                config: slack_config,
            }),
            _ => Outcome::Error((Status::Unauthorized, SlackParserError::MissingHeaders)),
        }
    }
}

impl SlackRequestParser {
    pub async fn parse_slash(&self, raw_data: Data<'_>) -> Result<SlashRequest, SlackParserError> {
        let data = raw_data
            // 10 KiB is enough for any reasonable request.
            .open(10.kibibytes())
            .into_string()
            .await
            .map_err(|_| SlackParserError::BadQueryString)?;
        if !data.is_complete() {
            return Err(SlackParserError::BadQueryString);
        }
        let request = serde_qs::from_str(&data).map_err(|_| SlackParserError::BadQueryString)?;

        // Verify timestamp.
        if let Some(max_age_seconds) = self.config.max_age_seconds {
            let request_time = Utc
                .timestamp_opt(self.timestamp, 0)
                .single()
                .ok_or(SlackParserError::BadQueryString)?;
            let current_time = chrono::Local::now();
            let difference = current_time.signed_duration_since(request_time).abs();
            // TODO: Move this to config parsing, so we store TimeDelta and crash early.
            if difference
                > chrono::TimeDelta::try_seconds(max_age_seconds)
                    .expect("Config max_age_seconds is invalid")
            {
                error!("Rejecting timestamp with large diff: {:?}", difference);
                return Err(SlackParserError::TimestampTooDifferent);
            }
        }

        // Verify signature.
        if let Some(ref secret) = self.config.secret {
            // Remove the version prefix and parse as hex.
            let signature = self.signature.trim_start_matches("v0=");
            let signature: [u8; 32] = hex::decode(signature)
                .map_err(|_| SlackParserError::SignatureInvalid)?
                .try_into()
                .map_err(|_| SlackParserError::SignatureInvalid)?;

            // Concat version, timestamp and data for HMAC.
            let basestring = format!("v0:{}:{}", self.timestamp, data.as_str());
            let hmac = hmac_sha256::HMAC::mac(basestring, secret);

            if !constant_time_eq::constant_time_eq_n(&hmac, &signature) {
                error!("Rejecting bad signature");
                return Err(SlackParserError::SignatureInvalid);
            }
        }

        Ok(request)
    }
}

// // From https://api.slack.com/docs/message-formatting
// fn slack_encode(s: &str) -> String {
//     s.replace("&", "&amp;")
//         .replace("<", "&lt;")
//         .replace(">", "&gt;")
// }

// fn post_slack_message(callback: &str, message: SlackMessage) -> Result<(), failure::Error> {
//     let client = reqwest::Client::new();
//     let response = client.post(callback).json(&message).send()?;
//     if !response.status().is_success() {
//         return Err(format_err!("Request failed: {:?}", response));
//     }
//     Ok(())
// }

// pub fn post_success(callback: &str, result: &RenderResult) -> Result<(), failure::Error> {
//     let mut text = String::new();
//     if result.user.is_some() {
//         writeln!(
//             &mut text,
//             ":bust_in_silhouette: {}",
//             result.user.as_ref().unwrap()
//         )
//         .unwrap();
//     }
//     writeln!(
//         &mut text,
//         ":page_with_curl: *{}*",
//         slack_encode(&result.title)
//     )
//     .unwrap();
//     writeln!(&mut text, ":lock: <{}|Original link>", result.orig_url).unwrap();
//     write!(&mut text, ":unlock: <{}|PDF version>", result.pdf_url).unwrap();
//     if result.png_url.is_some() {
//         write!(
//             &mut text,
//             "\n:camera: <{}|Screenshot>",
//             result.png_url.as_ref().unwrap()
//         )
//         .unwrap();
//     }

//     post_slack_message(
//         callback,
//         SlackMessage {
//             response_type: SlackResponseType::InChannel,
//             markdown: true,
//             text,
//         },
//     )
// }

// pub fn post_failure(callback: &str, error: &RenderError) -> Result<(), failure::Error> {
//     post_slack_message(
//         callback,
//         SlackMessage::ephemeral(format!("Error downloading: {}", error)),
//     )
// }
