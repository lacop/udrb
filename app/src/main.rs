#![feature(proc_macro_hygiene, decl_macro)]

extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate regex;
#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate url;

mod chrome;
mod config;
mod renderer;
mod slack;

use config::ConfigState;
use renderer::{RenderRequest, RenderSender, Renderer};
use slack::{SlackMessage, SlackRequestParser};

use rocket::data::Data;
use rocket::http::RawStr;
use rocket::response::status::BadRequest;
use rocket::response::NamedFile;
use rocket::State;
use rocket_contrib::json::Json;

#[get("/")]
fn index() -> &'static str {
    // TODO render index
    "UDRB is running..."
}

#[get("/<file..>")]
fn static_file(file: std::path::PathBuf, config: State<ConfigState>) -> Option<NamedFile> {
    // TODO return correct headers for mhtml files
    let path = config.get().output_dir.join(file);
    NamedFile::open(path).ok()
}

#[get("/fetch?<url>&<callback>")]
fn fetch(
    url: &RawStr,
    callback: Option<&RawStr>,
    sender: State<RenderSender>,
) -> Result<String, failure::Error> {
    let url = url::Url::parse(url.url_decode()?.as_str())?;
    let callback = callback
        .map(|s| s.url_decode().ok()
        .ok_or_else(|| format_err!("Invalid callback"))
    ).transpose()?;

    sender.render(RenderRequest {
        url: url.clone(),
        slack_callback: callback,
        user: None,
        channel: None,
        team: None,
    })?;

    Ok(format!("Fetching {:?} in the background", url))
}

#[post("/slash", data = "<data>")]
fn slash(
    parser: SlackRequestParser,
    data: Data,
    sender: State<RenderSender>,
) -> Result<Json<SlackMessage>, BadRequest<String>> {
    let request = parser
        .parse_slash(data)
        .map_err(|_| BadRequest(Some("Couldn't parse or verify request".to_string())))?;
    let (render_request, reply) = request.render_and_reply();
    if render_request.is_some() {
        sender
            .render(render_request.unwrap())
            .map_err(|_| BadRequest(Some("Internal error".to_string())))?;
    }
    Ok(Json(reply))
}

fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_state = config::ConfigState::from_env().expect("Error obtaining config");

    let sender = Renderer::start(&config_state.get()).expect("Failed to initialize renderer");

    rocket::ignite()
        .manage(config_state)
        .manage(sender)
        .mount("/", routes![index])
        .mount("/static", routes![static_file])
        .mount("/debug", routes![fetch])
        .mount("/slack", routes![slash])
        .launch();
}
