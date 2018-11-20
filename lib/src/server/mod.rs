use futures::future::lazy;
use futures::IntoFuture;
use lapin::client;
use model::ConnectionInfo;
use std::env;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use tokio::io;
use tokio::net::TcpStream;
use tokio::prelude::Future;

fn bootstrap(connection_info: ConnectionInfo) {
  debug!("Starting the server...");
  trace!(
    "Connecting to RabbitMQ on {}:*****@{}:{}/{}",
    connection_info.user,
    connection_info.host,
    connection_info.port,
    connection_info.vhost,
  );
  let client = create_client(connection_info);
  tokio::run(lazy(|| {
    debug!("Spawning tasks for each client");
    tokio::spawn(
      client
        .map(|_| {
          debug!("Successfully connected");
          ()
        }).map_err(|e| error!("Could not connect to RabbitMQ: {}", e)),
    );
    Ok(())
  }))
}

type ClientFuture = Box<Future<Item = client::Client<TcpStream>, Error = io::Error> + Send>;

fn create_client(connection_info: ConnectionInfo) -> ClientFuture {
  let amqp_connection = client::ConnectionOptions {
    username: connection_info.user,
    password: connection_info.password,
    vhost: connection_info.vhost,
    ..client::ConnectionOptions::default()
  };
  let addr = (&*connection_info.host, connection_info.port)
    .to_socket_addrs()
    .unwrap()
    .next()
    .unwrap();
  trace!("Opening a TCP connection to {}", addr);
  Box::new(
    TcpStream::connect(&addr)
      .and_then(|stream| client::Client::connect(stream, amqp_connection))
      .and_then(|(client, heartbeat)| {
        tokio::spawn(heartbeat.map_err(|e| warn!("heartbeat error: {:?}", e)));
        futures::future::ok(client)
      }).into_future(),
  )
}

// run should take a structure representing the imposters, and pass it to bootstrap which will be in charge of interpreting it
// for the moment, it is enough
pub fn run() {
  info!("running server");
  // TODO: get data from env for now
  // next, we will get the configuration from a file

  // finally: call bootstrap with imposter descriptions
  let rabbitmq_host =
    env::var("RABBITMQ_HOST").unwrap_or("amqp://guest:guest@localhost:5672/".to_owned()); // TODO: use a parameter instead
  let connection_info = ConnectionInfo::from_str(&rabbitmq_host)
    .expect("please set RABBITMQ_HOST correctly (amqp://user:password@host:port/vhost)");
  bootstrap(connection_info);
}
