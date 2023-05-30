use email_newsletter::{
    configuration::get_configuration,
    startup::run,
    telemetry::{get_subscriber, init_subscriber},
};
use sqlx::PgPool;
use std::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("email_newsletter".into(), "info".into());
    init_subscriber(subscriber);
    // Panic if we can't read configuration
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to DB");
    let port = configuration.application_port;
    let address = format!("{}:{}", configuration.application_host, port);
    let listener = TcpListener::bind(address).expect(&format!("Failed to bind to port {port}"));

    run(listener, connection_pool)?.await
}
