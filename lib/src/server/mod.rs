use failure::{err_msg, Error};
use futures::future::lazy;
use futures::future::Either;
use futures::IntoFuture;
use futures::Stream;
use lapin::channel;
use lapin::client;
use lapin::types::FieldTable;
use model::amqp::*;
use model::imposter::imposters::*;
use model::imposter::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::env;
use std::iter;
use std::net::ToSocketAddrs;
use std::str::FromStr;
use tokio::net::TcpStream;
use tokio::prelude::Future;

fn bootstrap(connection_info: &ConnectionInfo, bindings: &QueueBinding) {
  debug!("Starting the server...");
  trace!(
    "Connecting to RabbitMQ on {}:*****@{}:{}/{}",
    connection_info.user,
    connection_info.host,
    connection_info.port,
    connection_info.vhost,
  );
  let b = bindings.clone();
  let program = create_client(connection_info).and_then(move |client| create_imposter(&client, &b));

  tokio::run(lazy(|| {
    debug!("Spawning tasks for each client");
    tokio::spawn(
      program
        .map(|_| {
          debug!("Successfully connected");
        })
        .map_err(|e| error!("Could not connect to RabbitMQ: {}", e)),
    );
    Ok(())
  }))
}

// NOTE: the queue binding will eventually be wrapped into a reactor imposter config
fn create_imposter(
  client: &client::Client<TcpStream>,
  bindings: &QueueBinding,
) -> impl Future<Item = (), Error = Error> + Send + 'static {
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

  client
    .create_channel()
    .and_then(move |channel| {
      debug!("declaring queue {}", &queue_name);
      channel
        .queue_declare(
          &*queue_name,
          channel::QueueDeclareOptions::default(),
          FieldTable::new(),
        )
        .map(|queue| (channel, queue))
    })
    .and_then(move |(channel, queue)| {
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
        )
        .map(|_| (channel, queue))
    })
    .and_then(move |(channel, queue)| {
      debug!("creating a consumer for the imposter");
      channel
        .basic_consume(
          &queue,
          "",
          channel::BasicConsumeOptions::default(),
          FieldTable::new(),
        )
        .map(|stream| (channel, stream))
    })
    .and_then(move |(channel, stream)| {
      stream.for_each(move |message| {
        // message_printer should be a parameter of the function (*)
        let action =
          respond_hardcoded.react(InputMessage(std::str::from_utf8(&message.data).unwrap()));
        let c = channel.clone();
        interpret_action(&channel, action)
          .map(|_| c)
          .and_then(move |channel| channel.basic_ack(message.delivery_tag, false))
      })
    })
    .map_err(Error::from)
}

// TODO: extract this to an ActionInterpreter structure
fn interpret_action(
  channel: &channel::Channel<TcpStream>,
  action: Action,
) -> impl Future<Item = (), Error = lapin::error::Error> + Send + 'static {
  // interpret action is really a Action -> AmqpStuff function
  match action {
    Action::DoNothing => {
      debug!("Doing nothing");
      Either::A(futures::future::ok(()))
    }
    Action::SendMsg(MessageDispatch { message, .. }) => {
      info!("Publishing a message: {:?}", message);
      let mut rng = thread_rng(); // TODO: do not create it again and again (struct?)
      let message_id: String = iter::repeat(())
        .map(|_| rng.sample(Alphanumeric))
        .take(16)
        .collect(); // TODO: extract this to a function
      Either::B(
        channel
          .basic_publish(
            &*message.route.exchange,
            &*message.route.routing_key,
            message.body.0.into_bytes(),
            channel::BasicPublishOptions::default(),
            channel::BasicProperties::default()
              .with_message_id(message_id)
              .with_user_id("guest".to_string()),
          )
          .map(|_| ()),
      )
    }
    _ => {
      debug!("Don't know what to do");
      Either::A(futures::future::ok(()))
    }
  }
}

fn create_client(
  connection_info: &ConnectionInfo,
) -> impl Future<Item = client::Client<TcpStream>, Error = Error> + Send + 'static {
  let amqp_connection = client::ConnectionOptions {
    username: connection_info.user.clone(),
    password: connection_info.password.clone(),
    vhost: connection_info.vhost.clone(),
    ..client::ConnectionOptions::default()
  };
  let addr = (&*connection_info.host, connection_info.port)
    .to_socket_addrs()
    .unwrap()
    .next()
    .unwrap();
  trace!("Opening a TCP connection to {}", addr);
  TcpStream::connect(&addr)
    .map_err(Error::from)
    .and_then(|stream| client::Client::connect(stream, amqp_connection).map_err(Error::from))
    .and_then(|(client, heartbeat)| {
      tokio::spawn(heartbeat.map_err(|e| warn!("heartbeat error: {:?}", e)))
        .into_future()
        .map(|_| client)
        .map_err(|_| err_msg("Could not spawn the heartbeat"))
    })
}

// run should take a structure representing the imposters, and pass it to bootstrap which will be in charge of interpreting it
// for the moment, it is enough
pub fn run() {
  info!("running server");
  // TODO: get data from env for now
  // next, we will get the configuration from a file

  // finally: call bootstrap with imposter descriptions
  let rabbitmq_host =
    env::var("RABBITMQ_HOST").unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/".to_owned()); // TODO: use a parameter instead
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
  bootstrap(&connection_info, &queue_binding);
}
