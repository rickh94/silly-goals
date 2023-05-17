use actix_identity::IdentityMiddleware;
use actix_session::{storage::RedisActorSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, get, middleware::Compress, web, web::ServiceConfig, Responder};
use actix_web_static_files::ResourceFiles;
use askama_actix::Template;
use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor};
use shuttle_actix_web::ShuttleActixWeb;
use shuttle_secrets::SecretStore;
use silly_goals::{
    routes::{auth, dashboard},
    seed_db,
};
use sqlx::PgPool;
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

#[allow(unused)]
#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_shared_db::Postgres(
        local_uri = "postgres://rick:@localhost:5478/silly-goals"
    )] pool: PgPool,
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
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

    seed_db(&pool).await;

    let redis_uri = secret_store
        .get("REDIS_URI")
        .expect("REDIS_URI must be set");

    let hostname = secret_store.get("HOSTNAME").expect("HOSTNAME must be set");

    // TODO: make this nicer
    let rp_origin = Url::parse(&format!("https://{hostname}")).expect("Invalid URL");
    let builder = WebauthnBuilder::new(&hostname, &rp_origin)
        .expect("Invalid configuration")
        .rp_name("Actix-Polls");

    let webauthn = web::Data::new(builder.build().expect("Invalid configuration of webauthn"));

    let secret_key = secret_store
        .get("SECRET_KEY")
        .expect("SECRET_KEY must be set");

    let secret_key = Key::from(&secret_key.chars().map(|c| c as u8).collect::<Vec<u8>>());

    // SETUP EMAIL
    let smtp_user = secret_store
        .get("SMTP_USER")
        .expect("SMTP_USER must be set");
    let smtp_password = secret_store
        .get("SMTP_PASSWORD")
        .expect("SMTP_PASSWORD must be set");
    let smtp_host = secret_store
        .get("SMTP_HOST")
        .expect("SMTP_HOST must be set");
    let smtp_port: u16 = secret_store
        .get("SMTP_PORT")
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

    mailer
        .test_connection()
        .await
        .expect("Failed to connect to smtp server");

    let config = move |cfg: &mut ServiceConfig| {
        let generated = generate();
        cfg.service(ResourceFiles::new("/static", generated))
            .service(
                web::scope("")
                    .wrap(SessionMiddleware::new(
                        RedisActorSessionStore::new(redis_uri),
                        secret_key.clone(),
                    ))
                    .wrap(IdentityMiddleware::default())
                    .wrap(Compress::default())
                    .app_data(web::Data::new(pool.clone()))
                    .app_data(web::Data::new(mailer.clone()))
                    .service(about)
                    .service(auth::register)
                    .service(auth::post_register)
                    .service(auth::finish_registration)
                    .service(auth::login)
                    .service(auth::post_login)
                    .service(auth::finish_login)
                    .service(auth::profile)
                    .service(auth::logout)
                    .service(dashboard::dashboard)
                    .service(dashboard::new_group)
                    .service(dashboard::post_new_group)
                    .service(dashboard::get_group)
                    .service(index),
            );
    };
    // TODO: error handler to redirect unauthorized reqs

    Ok(config.into())
}
