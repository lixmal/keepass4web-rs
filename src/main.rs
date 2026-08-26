use clap::Parser;

use crate::config::config::Config;
use crate::server::server::Server;

mod auth_backend;
mod db_backend;
mod config;
mod server;
mod auth;
mod keepass;
mod rate_limit;
mod session;

const CONFIG_FILE: &str = "config.yml";


#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = CONFIG_FILE)]
    config: std::path::PathBuf,
    /// probe /health of a running instance and exit (for container healthchecks)
    #[arg(long)]
    health_check: bool,
}

#[actix_web::main]
async fn main() {
    let args = Args::parse();
    let config = Config::from_file(args.config).expect("Failed to parse config");

    if args.health_check {
        std::process::exit(health_check(config.port));
    }

    Server::new(config)
        .await.expect("Failed to start server")
}

// minimal http probe without a shell or curl, usable from a scratch image
fn health_check(port: u16) -> i32 {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let timeout = Duration::from_secs(3);
    for addr in ["127.0.0.1", "::1"] {
        let Ok(mut stream) = TcpStream::connect((addr, port)) else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));

        let request = format!("GET {} HTTP/1.0\r\nHost: localhost\r\n\r\n", server::route::ROUTE_HEALTH);
        if stream.write_all(request.as_bytes()).is_err() {
            continue;
        }

        let mut response = String::new();
        if stream.read_to_string(&mut response).is_ok() && response.starts_with("HTTP/1.0 200") {
            return 0;
        }
    }

    1
}
