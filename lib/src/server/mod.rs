use lapin::client;
use std::env;
use std::net::ToSocketAddrs;
use tokio::net::TcpStream;
use tokio::prelude::Future;

fn bootstrap(rabbitmq_host: String) {
  debug!("Starting the server...");
  debug!("Connecting to RabbitMQ on {}", rabbitmq_host);
  let addr = rabbitmq_host.to_socket_addrs().unwrap().next().unwrap(); // TODO: rabbitmq_host.parse().unwrap() ? || do not use localhost?

  let client =
    TcpStream::connect(&addr) // TODO: create a connection abstraction, and a create_connection function
      .and_then(|stream| client::Client::connect(stream, client::ConnectionOptions::default()))
      .and_then(|(client, heartbeat)| {
        tokio::spawn(heartbeat.map_err(|e| warn!("heartbeat error: {:?}", e)));
        futures::future::ok(client)
      });
  tokio::run(
    client
      .map(|_| {
        info!("Connection ok");
        ()
      }).map_err(|e| error!("{}", e)),
  )
}

pub fn run() {
  info!("running server");
  // TODO: get data from env; configure from file, call bootstrap with imposter descriptions
  let rabbitmq_host = env::var("RABBITMQ_HOST").unwrap_or("localhost:5672".to_owned()); // TODO: use a parameter instead
  bootstrap(rabbitmq_host);
}
