extern crate serde_qs;

use crate::config::{ConfigState, SlackConfig};
use crate::renderer::{RenderError, RenderRequest, RenderResult};

use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::{Outcome, State};

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
                SlackMessage {
                    response_type: SlackResponseType::Ephemeral,
                    text: "Unknown command. Use /udrb http://...".to_string(),
                },
            );
        }

        let url = url::Url::parse(&self.text).ok();
        if url.is_none() {
            return (
                None,
                SlackMessage {
                    response_type: SlackResponseType::Ephemeral,
                    text: "Invalid arguments. Use /udrb http://...".to_string(),
                },
            );
        }

        let request = RenderRequest {
            url: url.unwrap(),
            slack_callback: Some(self.response_url),
        };
        let reply = SlackMessage {
            response_type: SlackResponseType::Ephemeral,
            text: "Downloading...".to_string(),
        };
        (Some(request), reply)
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

pub fn post_success(callback: &str, result: &RenderResult) -> Result<(), failure::Error> {
    println!("SLACK OK {:?} {:?}", callback, result);
    return Ok(());
}

pub fn post_failure(callback: &str, error: &RenderError) -> Result<(), failure::Error> {
    println!("SLACK FAIL {:?} {}", callback, error);
    return Ok(());
}
