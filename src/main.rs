use std::time::Duration;

use actix_identity::IdentityMiddleware;
use actix_session::{storage::RedisSessionStore, SessionMiddleware};
use actix_web::{
    cookie::Key,
    get,
    http::StatusCode,
    middleware::{Compress, ErrorHandlers, Logger},
    web, App, HttpServer, Responder,
};
use actix_web_static_files::ResourceFiles;
use askama_actix::Template;
use env_logger::Env;
use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor};
use log::info;
use silly_goals::{
    handle_unauthorized,
    routes::{auth, dashboard, webauthn_routes},
    seed_db,
};
use sqlx::sqlite::SqlitePool;
use webauthn_rs::prelude::*;

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

#[derive(Template)]
#[template(path = "about.html")]
struct AboutPage<'a> {
    pub title: &'a str,
}

#[get("/about")]
async fn about() -> impl Responder {
    AboutPage {
        title: "About Silly Goals",
    }
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    dotenvy::dotenv().ok();

    let db_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
    //     info!("Creating sqlite database");
    //     Sqlite::create_database(&db_url)
    //         .await
    //         .expect("Could not create database");
    // }

    let pool = SqlitePool::connect(&db_url)
        .await
        .expect("Could not connect to database");

    info!("Running migrations");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|err| {
            dbg!(err);
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Database migration was unsuccessful",
            )
        })?;

    info!("Seeding Database");
    seed_db(&pool).await;

    let redis_uri = dotenvy::var("REDIS_URL").expect("REDIS_URL must be set");

    let hostname = dotenvy::var("HOSTNAME").expect("HOSTNAME must be set");

    let rp_origin = Url::parse(&format!("https://{hostname}")).expect("Invalid URL");
    let builder = WebauthnBuilder::new(&hostname, &rp_origin)
        .expect("Invalid configuration")
        .rp_name("Silly Goals");

    let webauthn = web::Data::new(builder.build().expect("Invalid configuration of webauthn"));

    let secret_key = dotenvy::var("SECRET_KEY").expect("SECRET_KEY must be set");

    let secret_key = Key::from(&secret_key.chars().map(|c| c as u8).collect::<Vec<u8>>());

    // SETUP EMAIL
    let smtp_user = dotenvy::var("SMTP_USER").expect("SMTP_USER must be set");
    let smtp_password = dotenvy::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set");
    let smtp_host = dotenvy::var("SMTP_HOST").expect("SMTP_HOST must be set");
    let smtp_port: u16 = dotenvy::var("SMTP_PORT")
        .expect("SMTP_PORT must be set")
        .parse()
        .expect("SMTP_PORT must a a valid port number");
    let creds = Credentials::new(smtp_user, smtp_password);

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host.as_str())
            .expect("could not setup smtp connection")
            .port(smtp_port)
            .credentials(creds)
            .build();

    info!("Testing mailer");
    mailer
        .test_connection()
        .await
        .expect("Failed to connect to smtp server");

    info!("Connecting to redis");
    let redis_store = RedisSessionStore::new(redis_uri)
        .await
        .expect("to connect to redis store");

    let bind_address = if dotenvy::var("DEBUG").is_ok() {
        "127.0.0.1"
    } else {
        "0.0.0.0"
    };

    info!("Creating server");
    HttpServer::new(move || {
        let generated = generate();
        App::new()
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            .wrap(
                IdentityMiddleware::builder()
                    .visit_deadline(Some(Duration::from_secs(3600 * 24)))
                    .build(),
            )
            .wrap(Compress::default())
            .wrap(ErrorHandlers::new().handler(StatusCode::UNAUTHORIZED, handle_unauthorized))
            .wrap(Logger::default())
            .app_data(webauthn.clone())
            .app_data(web::Data::new(pool.clone()))
            .service(ResourceFiles::new("/static", generated))
            .app_data(web::Data::new(mailer.clone()))
            .service(about)
            .service(auth::register)
            .service(auth::post_register)
            .service(auth::finish_registration)
            .service(auth::login)
            .service(auth::login_with_code)
            .service(auth::post_login)
            .service(auth::finish_login)
            .service(auth::profile)
            .service(auth::profile_edit_name)
            .service(auth::post_profile_edit_name)
            .service(auth::profile_edit_email)
            .service(auth::post_profile_edit_email)
            .service(auth::post_profile_confirm_email)
            .service(auth::delete_profile)
            .service(auth::logout)
            .service(dashboard::dashboard)
            .service(dashboard::new_group)
            .service(dashboard::post_new_group)
            .service(dashboard::dashboard_edit_group)
            .service(dashboard::group_edit_group)
            .service(dashboard::delete_group)
            .service(dashboard::post_edit_group)
            .service(dashboard::get_group)
            .service(dashboard::new_goal)
            .service(dashboard::post_new_goal)
            .service(dashboard::get_goal)
            .service(dashboard::edit_goal)
            .service(dashboard::post_edit_goal)
            .service(dashboard::patch_goal_tone)
            .service(dashboard::delete_goal)
            .service(webauthn_routes::start_registration)
            .service(webauthn_routes::finish_registration)
            .service(webauthn_routes::start_login)
            .service(webauthn_routes::finish_login)
            .service(index)
    })
    .bind((bind_address, 8000))?
    .run()
    .await
}
