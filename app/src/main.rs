#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate rocket;
extern crate rocket_contrib;
extern crate url;

mod chrome;
mod config;
mod renderer;
mod slack;

use renderer::{RenderRequest, RenderSender, Renderer};
use slack::{SlackMessage, SlackRequestParser};

use rocket::http::RawStr;
use rocket::response::status::BadRequest;
use rocket::State;
use rocket_contrib::json::Json;

#[get("/")]
fn index() -> &'static str {
    // TODO render index
    "Hello, world!"
}

#[get("/fetch?<url>&<callback>")]
fn fetch(
    url: &RawStr,
    callback: Option<&RawStr>,
    sender: State<RenderSender>,
) -> Result<String, failure::Error> {
    let url = url::Url::parse(url.url_decode()?.as_str())?;
    let callback = callback
        .map(|s| s.url_decode().ok())
        .ok_or(format_err!("Invalid callback"))?;

    sender.render(RenderRequest {
        url: url.clone(),
        slack_callback: callback,
    })?;

    Ok(format!("Fetching {:?} in the background", url))
}

#[post("/slash", data = "<data>")]
fn slash(
    parser: SlackRequestParser,
    data: String,
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
    let config_state = config::ConfigState::from_evn().expect("Error obtaining config");

    let sender = Renderer::start(&config_state.get()).expect("Failed to initialize renderer");

    rocket::ignite()
        .manage(config_state)
        .manage(sender)
        .mount("/", routes![index])
        .mount("/debug", routes![fetch])
        .mount("/slack", routes![slash])
        .launch();

    //let mut chrome = chrome::ChromeDriver::new().unwrap();
    //chrome.navigate("https://predplatne.dennikn.sk/sign/in/").unwrap();
    //chrome.foo();
    //chrome.navigate("https://dennikn.sk/1411992/vo-volebnej-komisii-v-petrzalke-sedi-stefan-agh-obvineny-s-kocnerom-z-falsovania-zmeniek").unwrap();
    //chrome.save_screenshot(std::path::Path::new("/tmp/output")).unwrap();
    //chrome.save_pdf(std::path::Path::new("/tmp/output")).unwrap();
}
