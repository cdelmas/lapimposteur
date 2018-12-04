use failure::{err_msg, Error};
use futures::future::lazy;
use futures::IntoFuture;
use futures::Stream;
use lapin::channel;
use lapin::client;
use lapin::types::FieldTable;
use model::amqp::*;
use model::imposter::imposters::*;
use model::imposter::*;
use std::env;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use tokio::net::TcpStream;
use tokio::prelude::Future;

fn bootstrap(connection_info: ConnectionInfo, bindings: QueueBinding) {
  debug!("Starting the server...");
  trace!(
    "Connecting to RabbitMQ on {}:*****@{}:{}/{}",
    connection_info.user,
    connection_info.host,
    connection_info.port,
    connection_info.vhost,
  );
  let program =
    create_client(connection_info).and_then(|client| create_imposter(&client, bindings));

  tokio::run(lazy(|| {
    debug!("Spawning tasks for each client");
    tokio::spawn(
      program
        .map(|_| {
          debug!("Successfully connected");
          ()
        }).map_err(|e| error!("Could not connect to RabbitMQ: {}", e)),
    );
    Ok(())
  }))
}

type ImposterFuture = Box<Future<Item = (), Error = Error> + Send>;

// NOTE: the queue binding will eventually be wrapped into a reactor imposter config
fn create_imposter(client: &client::Client<TcpStream>, bindings: QueueBinding) -> ImposterFuture {
  let queue_name = bindings.queue_name.0.clone();
  let exchange_name = bindings.exchange_name.0.clone();
  let routing_key = bindings.routing_key.0.clone();

  let respond_hardcoded = FnReactor::new(|message| {
    info!("{}", message.0);
    Action::SendMsg(MessageDispatch {
      message: Message::new(
        Route::new("rx", "x.y.z"),
        Body(String::from("some text")),
        Headers::empty(),
      ),
      delay: Schedule::Now,
    })
  });

  Box::new(
    client
      .create_channel()
      .and_then(move |channel| {
        debug!("declaring queue {}", &queue_name);
        channel
          .queue_declare(
            &*queue_name,
            channel::QueueDeclareOptions::default(),
            FieldTable::new(),
          ).map(|queue| (channel, queue))
      }).and_then(move |(channel, queue)| {
        debug!(
          "binding queue {} -{}-> {}",
          queue.name(),
          &routing_key,
          &exchange_name
        );
        channel
          .queue_bind(
            &queue.name(),
            &*exchange_name,
            &*routing_key,
            channel::QueueBindOptions::default(),
            FieldTable::new(),
          ).map(|_| (channel, queue))
      }).and_then(move |(channel, queue)| {
        debug!("creating a consumer for the imposter");
        channel
          .basic_consume(
            &queue,
            "",
            channel::BasicConsumeOptions::default(),
            FieldTable::new(),
          ).map(|stream| (channel, stream))
      }).and_then(move |(channel, stream)| {
        stream.for_each(move |message| {
          // message_printer should be a parameter of the function (*)
          let action =
            respond_hardcoded.react(InputMessage(std::str::from_utf8(&message.data).unwrap()));
          interpret_action(channel.clone(), action)
            .and_then(move |channel| channel.basic_ack(message.delivery_tag, false))
        })
      }).map_err(Error::from),
  )
}

type ClientFuture = Box<Future<Item = client::Client<TcpStream>, Error = Error> + Send>;

fn interpret_action(
  channel: channel::Channel<TcpStream>,
  action: Action,
) -> Box<Future<Item = channel::Channel<TcpStream>, Error = std::io::Error> + Send> {
  // interpret action is really a Action -> AmqpStuff function
  match action {
    Action::DoNothing => {
      debug!("Doing nothing");
      Box::new(futures::future::ok(channel))
    }
    Action::SendMsg(MessageDispatch { message, delay: _ }) => {
      info!("Publishing a message: {:?}", message);
      Box::new(
        channel
          .basic_publish(
            &*message.route.exchange,
            &*message.route.routing_key,
            message.body.0.into_bytes(),
            channel::BasicPublishOptions::default(),
            channel::BasicProperties::default().with_user_id("guest".to_string()),
          ).map(|_| channel),
      )
    }
    _ => {
      debug!("Don't know what to do");
      Box::new(futures::future::ok(channel))
    }
  }
}

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
      .map_err(Error::from)
      .and_then(|stream| client::Client::connect(stream, amqp_connection).map_err(Error::from))
      .and_then(|(client, heartbeat)| {
        tokio::spawn(heartbeat.map_err(|e| warn!("heartbeat error: {:?}", e)))
          .into_future()
          .map(|_| client)
          .map_err(|_| err_msg("Could not spawn the heartbeat"))
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
  let queue_name = env::var("QUEUE").expect("please set QUEUE");
  let exchange_name = env::var("EXCHANGE").expect("please set EXCHANGE");
  let binding = env::var("BINDING").expect("please set BINDING");
  let queue_binding = QueueBinding::new(
    QueueName(queue_name),
    RoutingKey(binding),
    ExchangeName(exchange_name),
  );
  debug!("{:?}", queue_binding);
  let connection_info = ConnectionInfo::from_str(&rabbitmq_host)
    .expect("please set RABBITMQ_HOST correctly (amqp://user:password@host:port/vhost)");
  bootstrap(connection_info, queue_binding);
}
