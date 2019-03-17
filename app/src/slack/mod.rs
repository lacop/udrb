extern crate serde_qs;

use crate::config::{ConfigState, SlackConfig};
use crate::renderer::{RenderError, RenderRequest, RenderResult};

use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::{Outcome, State};

use std::fmt::Write;

#[derive(Debug, Deserialize)]
pub struct SlashRequest {
    command: String,
    text: String,
    response_url: String,
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
            text: text,
            markdown: false,
        }
    }
}

pub struct SlackRequestParser {
    timestamp: String,
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
        let signature = req.headers().get_one("X-Slack-Signature");
        match (timestamp, signature) {
            (Some(t), Some(s)) => Outcome::Success(SlackRequestParser {
                timestamp: t.to_string(),
                signature: s.to_string(),
                config: slack_config,
            }),
            _ => Outcome::Failure((Status::Unauthorized, SlackParserError::MissingHeaders)),
        }
    }
}

impl SlackRequestParser {
    pub fn parse_slash(&self, data: String) -> Result<SlashRequest, SlackParserError> {
        let request = serde_qs::from_str(&data).map_err(|_| SlackParserError::BadQueryString)?;
        // TODO verify timestamp
        // TODO verify signature
        Ok(request)
    }
}

// From https://api.slack.com/docs/message-formatting
fn slack_encode(s: &String) -> String {
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
    let mut text = format!(":page_with_curl: *{}*\n", slack_encode(&result.title));
    write!(&mut text, ":lock: <{}|Original link>\n", result.orig_url).unwrap();
    write!(&mut text, ":unlock: <{}|PDF version>", result.pdf_url).unwrap();

    post_slack_message(
        callback,
        SlackMessage {
            response_type: SlackResponseType::InChannel,
            markdown: true,
            text: text,
        },
    )
}

pub fn post_failure(callback: &str, error: &RenderError) -> Result<(), failure::Error> {
    post_slack_message(
        callback,
        SlackMessage::ephemeral(format!("Error downloading: {}", error)),
    )
}
