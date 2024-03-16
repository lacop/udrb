use crate::config::{Config, SlackConfig};
use crate::renderer::{RenderError, RenderRequest, RenderResult};

use chrono::{TimeZone, Utc};
use log::error;
use rocket::data::{Data, ToByteUnit};
use rocket::http::Status;
use rocket::request::Outcome;
use rocket::request::{self, FromRequest, Request};
use rocket::serde::json;
use rocket::State;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SlashRequest {
    command: String,
    text: String,
    response_url: String,
    user_id: Option<String>,
    channel_name: Option<String>,
    team_domain: Option<String>,
}

impl SlashRequest {
    pub fn render_and_reply(self) -> (Option<RenderRequest>, SlackMessage) {
        let usage = SlackMessage {
            response_type: SlackResponseType::Ephemeral,
            blocks: vec![SlackBlock {
                type_: "context".to_string(),
                elements: vec![SlackBlockElement {
                    type_: "mrkdwn".to_string(),
                    text: Some("Bad request. Usage: `/udrb http://...`".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        if self.command != "/udrb" {
            return (None, usage);
        }

        let url = match url::Url::parse(&self.text) {
            Ok(url) => url,
            Err(_) => {
                return (None, usage);
            }
        };

        (
            Some(RenderRequest {
                url,
                slack_callback: self.response_url,
                user: self.user_id,
                channel: self.channel_name,
                team: self.team_domain,
            }),
            SlackMessage {
                response_type: SlackResponseType::Ephemeral,
                blocks: vec![SlackBlock {
                    type_: "context".to_string(),
                    elements: vec![SlackBlockElement {
                        type_: "mrkdwn".to_string(),
                        text: Some("_Downloading, please wait..._".to_string()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
            },
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackResponseType {
    Ephemeral,
    InChannel,
}

#[derive(Debug, Serialize, Default)]
pub struct SlackBlock {
    #[serde(rename = "type")]
    type_: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    elements: Vec<SlackBlockElement>,

    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<SlackTextBlock>,
}

#[derive(Debug, Serialize, Default)]
pub struct SlackBlockElement {
    #[serde(rename = "type")]
    type_: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    alt_text: Option<String>,

    // This is silly but Slack API has "text" be either string or nested message.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "text")]
    button_text: Option<SlackButtonText>,

    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct SlackButtonText {
    #[serde(rename = "type")]
    type_: String,
    text: String,
    emoji: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct SlackTextBlock {
    #[serde(rename = "type")]
    type_: String,
    text: String,
}

#[derive(Debug, Serialize)]
pub struct SlackMessage {
    response_type: SlackResponseType,
    blocks: Vec<SlackBlock>,
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
        let request_time = Utc
            .timestamp_opt(self.timestamp, 0)
            .single()
            .ok_or(SlackParserError::BadQueryString)?;
        let current_time = chrono::Local::now();
        let difference = current_time.signed_duration_since(request_time).abs();
        if difference > self.config.max_age {
            error!("Rejecting timestamp with large diff: {:?}", difference);
            return Err(SlackParserError::TimestampTooDifferent);
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

fn post_slack_message(callback: &str, message: SlackMessage) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::new();
    println!("{}", json::to_string(&message).unwrap());
    let response = client.post(callback).json(&message).send()?;
    if !response.status().is_success() {
        return Err(anyhow::format_err!("Request failed: {:?}", response));
    }
    Ok(())
}

// TODO: Replace with Block Kit (new fancy message format).
pub fn post_success(callback: &str, result: &RenderResult) -> anyhow::Result<()> {
    // let mut text = String::new();
    // if let Some(ref user) = result.user {
    //     writeln!(&mut text, ":bust_in_silhouette: <@{user}>").unwrap();
    // }
    // writeln!(
    //     &mut text,
    //     ":page_with_curl: *{}*",
    //     slack_encode(&result.title)
    // )
    // .unwrap();
    // writeln!(&mut text, ":lock: <{}|Original link>", result.orig_url).unwrap();
    // write!(&mut text, ":unlock: <{}|PDF version>", result.pdf_url).unwrap();
    // if let Some(ref png_url) = result.png_url {
    //     write!(&mut text, "\n:camera: <{}|Screenshot>", png_url).unwrap();
    // }

    let mut response_blocks = Vec::new();

    // Header with the page title.
    response_blocks.push(SlackBlock {
        type_: "header".to_string(),
        text: Some(SlackTextBlock {
            type_: "plain_text".to_string(),
            text: result.title.clone(),
        }),
        ..Default::default()
    });
    // TODO: Maybe extract title from page, not <title>?
    // TODO: Try extracting summary (first paragraph) to show here.
    // TODO: Extract whole article without ads/menus etc ("reader view"),
    //       use that to show the length / reading time, TTS narration, etc.

    // Buttons with links to all the versions.
    let mut buttons_block = SlackBlock {
        type_: "actions".to_string(),
        elements: vec![],
        ..Default::default()
    };
    buttons_block.elements.push(SlackBlockElement {
        type_: "button".to_string(),
        button_text: Some(SlackButtonText {
            type_: "plain_text".to_string(),
            text: ":lock: Original".to_string(),
            emoji: true,
        }),
        url: Some(result.orig_url.to_string()),
        ..Default::default()
    });
    buttons_block.elements.push(SlackBlockElement {
        type_: "button".to_string(),
        button_text: Some(SlackButtonText {
            type_: "plain_text".to_string(),
            text: ":unlock: PDF".to_string(),
            emoji: true,
        }),
        url: Some(result.pdf_url.to_string()),
        ..Default::default()
    });
    if let Some(ref png_url) = result.png_url {
        buttons_block.elements.push(SlackBlockElement {
            type_: "button".to_string(),
            button_text: Some(SlackButtonText {
                type_: "plain_text".to_string(),
                text: ":camera: Screenshot".to_string(),
                emoji: true,
            }),
            url: Some(png_url.to_string()),
            ..Default::default()
        });
    }
    response_blocks.push(buttons_block);

    // Page favicon and user who requested it.
    let mut favicon_and_user = SlackBlock {
        type_: "context".to_string(),
        elements: vec![],
        ..Default::default()
    };
    if let Some(host) = result.orig_url.host_str() {
        favicon_and_user.elements.push(SlackBlockElement {
            type_: "image".to_string(),
            image_url: Some(format!(
                "{}://{}/favicon.ico",
                result.orig_url.scheme(),
                host
            )),
            alt_text: Some(host.to_owned()),
            ..Default::default()
        });
    }
    if let Some(ref user) = result.user {
        favicon_and_user.elements.push(SlackBlockElement {
            type_: "mrkdwn".to_string(),
            text: Some(format!("Shared by <@{}>", user)),
            ..Default::default()
        });
    }
    if !favicon_and_user.elements.is_empty() {
        response_blocks.push(favicon_and_user);
    }

    post_slack_message(
        callback,
        SlackMessage {
            response_type: SlackResponseType::InChannel,
            blocks: response_blocks,
        },
    )
}

pub fn post_failure(callback: &str, error: &RenderError) -> anyhow::Result<()> {
    post_slack_message(
        callback,
        SlackMessage {
            response_type: SlackResponseType::Ephemeral,
            blocks: vec![SlackBlock {
                type_: "context".to_string(),
                elements: vec![SlackBlockElement {
                    type_: "mrkdwn".to_string(),
                    text: Some(format!("Error downloading: {}", error)),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        },
    )
}
