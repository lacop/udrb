mod chrome;
mod config;
mod renderer;
mod slack;

use renderer::{RenderSender, Renderer};
use slack::{SlackMessage, SlackRequestParser};

use rocket::response::status::BadRequest;
use rocket::serde::json::Json;

#[rocket::get("/")]
fn index() -> &'static str {
    "UDRB is running..."
}

#[rocket::post("/slash", data = "<data>")]
async fn slash(
    parser: SlackRequestParser,
    data: rocket::Data<'_>,
    sender: &rocket::State<RenderSender>,
) -> Result<Json<SlackMessage>, BadRequest<&'static str>> {
    let request = parser
        .parse_slash(data)
        .await
        .map_err(|_| BadRequest("Couldn't parse or verify request"))?;
    let (render_request, reply) = request.render_and_reply();
    if let Some(request) = render_request {
        // Not async, but the queue is unbounded.
        sender
            .render(request)
            .map_err(|_| BadRequest("Internal error"))?;
    }
    Ok(Json(reply))
}

// Using Block Kit with buttons requires an interactive endpoint to
// be configured and return HTTP 200. Otherwise we get a warning next
// to the message (the buttons still work, but it looks bad).
// See https://github.com/slackapi/node-slack-sdk/issues/869.
#[rocket::post("/interactive")]
fn interactive() -> Result<(), ()> {
    Ok(())
}

#[rocket::launch]
fn rocket() -> _ {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = config::Config::from_env().expect("Error obtaining config");
    let output_dir = config.output_dir.clone();

    let sender = Renderer::start(&config).expect("Failed to initialize renderer");

    rocket::build()
        .manage(config)
        .manage(sender)
        .mount("/", rocket::routes![index])
        .mount("/static", rocket::fs::FileServer::from(output_dir))
        .mount("/slack", rocket::routes![slash, interactive])
}
