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

mod chrome;
mod renderer;

use renderer::{RenderRequest, RenderSender, Renderer};

use rocket::http::RawStr;
use rocket::State;

#[get("/")]
fn index() -> &'static str {
    // TODO render index
    "Hello, world!"
}

#[get("/fetch?<url>")]
fn fetch(url: &RawStr, sender: State<RenderSender>) -> String {
    let req = RenderRequest { url: url.as_str().to_string(), slack_callback: None };
    sender.render(req).unwrap();
    format!("Fetching {} in the background", url.as_str())
}

fn main() {
    let sender = Renderer::start().unwrap();

    rocket::ignite()
        .manage(sender)
        .mount("/", routes![index])
        .mount("/debug", routes![fetch])
        //.mount("/slack", routes![slash])
        .launch();
}
