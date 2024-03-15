// mod chrome;
mod config;
mod renderer;
mod slack;

// use renderer::{RenderRequest, RenderSender, Renderer};
use slack::{SlackMessage, SlackRequestParser};

use rocket::response::status::BadRequest;
use rocket::serde::json::Json;

#[rocket::get("/")]
fn index() -> &'static str {
    // TODO: Some sort of fancier index?
    "UDRB is running..."
}

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
        // TODO: Use async MPSC?
        // sender
        //     .render(render_request.unwrap())
        //     .map_err(|_| BadRequest(Some("Internal error".to_string())))?;
    }
    Ok(Json(reply))
}

#[rocket::launch]
fn rocket() -> _ {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = config::Config::from_env().expect("Error obtaining config");
    let output_dir = config.output_dir.clone();

    //     let sender = Renderer::start(&config_state.get()).expect("Failed to initialize renderer");

    rocket::build()
        .manage(config)
        .mount("/", rocket::routes![index])
        .mount("/static", rocket::fs::FileServer::from(output_dir))
        .mount("/slack", rocket::routes![slash])
}
