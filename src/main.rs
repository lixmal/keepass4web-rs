use clap::Parser;

use crate::config::config::Config;
use crate::server::server::Server;

mod auth_backend;
mod db_backend;
mod config;
mod server;
mod auth;
mod keepass;
mod session;

const CONFIG_FILE: &str = "config.yml";


#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = CONFIG_FILE)]
    config: std::path::PathBuf,
}

#[actix_web::main]
async fn main() {
    let args = Args::parse();
    let config = Config::from_file(args.config).expect("Failed to parse config");

    Server::new(config).await.expect("Failed to start server")
        .await.expect("Failed to stop server");
}
