use actix_web::error::{ErrorBadRequest, ErrorInternalServerError};
use anyhow::anyhow;
use lettre::{message::Mailbox, Address, Message};
use log::error;

pub fn parse_email_to_mailbox(email: &str) -> actix_web::Result<Mailbox> {
    let email_address = email.parse::<Address>().map_err(|err| {
        error!("Error parsing user email from {}. Error: {}", email, err);
        ErrorBadRequest(anyhow!("Could not parse email"))
    })?;
    Ok(Mailbox::new(None, email_address))
}

pub fn build_plain_email(
    user_mailbox: Mailbox,
    subject: &str,
    body: &str,
) -> actix_web::Result<Message> {
    Message::builder()
        .from(
            "Silly Goals <sillygoals@mg.rickhenry.dev>"
                .parse()
                .expect("Invalid outgoing email"),
        )
        .to(user_mailbox)
        .subject(subject)
        .body(body.to_owned())
        .map_err(|err| {
            error!("Could not construct email: {}", err);
            ErrorInternalServerError(err)
        })
}

pub fn build_email_for_user(email: &str, subject: &str, body: &str) -> actix_web::Result<Message> {
    let user_mailbox = parse_email_to_mailbox(email)?;

    build_plain_email(user_mailbox, subject, body)
}
