use actix_web::{get, web::ServiceConfig, Responder};
use actix_web_static_files::ResourceFiles;
use askama_actix::Template;
use shuttle_actix_web::ShuttleActixWeb;

#[derive(Template)]
#[template(path = "index.html")]
struct HomePage<'a> {
    pub title: &'a str,
}

#[get("/")]
async fn index() -> impl Responder {
    HomePage {
        title: "Silly Goals",
    }
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[allow(unused)]
#[shuttle_runtime::main]
async fn actix_web() -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
    let config = move |cfg: &mut ServiceConfig| {
        let generated = generate();
        cfg.service(index)
            .service(ResourceFiles::new("/static", generated));
    };

    Ok(config.into())
}
