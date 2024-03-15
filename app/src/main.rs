// #![feature(proc_macro_hygiene, decl_macro)]

// extern crate env_logger;
// #[macro_use]
// extern crate failure;
// extern crate log;
// #[macro_use]
// extern crate serde_derive;
// #[macro_use]
// extern crate serde_json;
// extern crate regex;
// #[macro_use]
// extern crate rocket;
// extern crate rocket_contrib;
// extern crate url;

// mod chrome;
mod config;
mod renderer;
mod slack;

// use renderer::{RenderRequest, RenderSender, Renderer};
use slack::{SlackMessage, SlackRequestParser};

// use rocket::data::Data;
// use rocket::http::RawStr;
use rocket::response::status::BadRequest;
use rocket::serde::json::Json;

#[rocket::get("/")]
fn index() -> &'static str {
    // TODO render index
    "UDRB is running..."
}

// #[get("/fetch?<url>&<callback>")]
// fn fetch(
//     url: &RawStr,
//     callback: Option<&RawStr>,
//     sender: State<RenderSender>,
// ) -> Result<String, failure::Error> {
//     let url = url::Url::parse(url.url_decode()?.as_str())?;
//     let callback = callback
//         .map(|s| s.url_decode().ok()
//         .ok_or_else(|| format_err!("Invalid callback"))
//     ).transpose()?;

//     sender.render(RenderRequest {
//         url: url.clone(),
//         slack_callback: callback,
//         user: None,
//         channel: None,
//         team: None,
//     })?;

//     Ok(format!("Fetching {:?} in the background", url))
// }

#[rocket::post("/slash", data = "<data>")]
async fn slash(
    parser: SlackRequestParser,
    data: rocket::Data<'_>,
    //sender: State<RenderSender>,
) -> Result<Json<SlackMessage>, BadRequest<&'static str>> {
    let request = parser
        .parse_slash(data)
        .await
        .map_err(|_| BadRequest("Couldn't parse or verify request"))?;
    let (render_request, reply) = request.render_and_reply();
    if render_request.is_some() {
        // sender
        //     .render(render_request.unwrap())
        //     .map_err(|_| BadRequest(Some("Internal error".to_string())))?;
    }
    Ok(Json(reply))
}

#[rocket::launch]
fn rocket() -> _ {
    //     env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = config::Config::from_env().expect("Error obtaining config");
    let output_dir = config.output_dir.clone();

    //     let sender = Renderer::start(&config_state.get()).expect("Failed to initialize renderer");

    rocket::build()
        .manage(config)
        .mount("/", rocket::routes![index])
        .mount("/static", rocket::fs::FileServer::from(output_dir))
        .mount("/slack", rocket::routes![slash])
    //     rocket::ignite()
    //         .manage(config_state)
    //         .manage(sender)
    //         .mount("/", routes![index])
    //         .mount("/static", routes![static_file])
    //         .mount("/debug", routes![fetch])
    //         .mount("/slack", routes![slash])
    //         .launch();
}
