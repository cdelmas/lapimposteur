use cron::Schedule;
use failure::{err_msg, Error};
use futures::future::lazy;
use futures::IntoFuture;
use futures::Sink;
use futures::Stream;
use lapin_futures_rustls::{
  lapin, lapin::channel::*, lapin::message::Delivery, lapin::types::*, AMQPConnectionRustlsExt,
  AMQPStream,
};
use model::imposter::{Lit::*, *};
use rand::thread_rng;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::prelude::Future;
use tokio::timer::Delay;

fn bootstrap(imposter: Imposter) {
  tokio::run(lazy(|| {
    let connection_str = imposter.connection.clone();
    let program = create_client(&connection_str)
      .and_then(move |client| {
        create_client(&connection_str).map(|publish_client| (client, publish_client))
      })
      .and_then(move |(client, publish_client)| {
        futures::stream::iter_ok(imposter.reactors)
          .for_each(move |reactor| {
            tokio::spawn(
              create_reactor(&client, &publish_client, reactor)
                .map_err(|e| error!("Reactor error: {}", e)),
            )
          })
          .into_future()
          .map_err(|_| err_msg("Couldn't spawn the consumer task"))
          .map(move |_| {
            warn!("End of reactor stream");
            ()
          })
      });

    tokio::spawn(
      program
        .map_err(move |e| error!("Could not connect to RabbitMQ: {}", e))
        .map(|_| {
          debug!("Successfully connected");
        }),
    );
    Ok(())
  }));
}

fn create_reactor(
  client: &lapin::client::Client<AMQPStream>,
  publish_client: &lapin::client::Client<AMQPStream>,
  reactor: ReactorSpec,
) -> impl Future<Item = (), Error = Error> {
  let rk = reactor.routing_key.clone();
  let xchg = reactor.exchange.clone();
  let q = reactor.queue.clone();
  let action = reactor.action.clone();
  let client = client.clone();
  let publish_client = publish_client.clone();
  client
    .create_channel()
    .map_err(Error::from)
    .and_then(move |channel| {
      publish_client
        .create_channel()
        .map(|publisher_channel| (publisher_channel, channel))
        .map_err(Error::from)
    })
    .and_then(move |(publisher_channel, channel)| {
      debug!("Declaring queue {}", &q);
      channel
        .queue_declare(
          &q,
          QueueDeclareOptions {
            auto_delete: true,
            ..Default::default()
          },
          FieldTable::new(),
        )
        .map(move |queue| (publisher_channel, channel, queue))
        .map_err(Error::from)
    })
    .and_then(move |(publisher_channel, channel, queue)| {
      debug!("Binding {}======{}=====>{}", &queue.name(), &rk, &xchg);
      channel
        .queue_bind(
          &queue.name(),
          &xchg,
          &rk,
          QueueBindOptions::default(),
          FieldTable::new(),
        )
        .map(move |_| (publisher_channel, channel, queue))
        .map_err(Error::from)
    })
    .and_then(move |(publisher_channel, channel, queue)| {
      debug!("Consuming on {}", &queue.name());
      channel
        .basic_consume(
          &queue,
          "", // consumer tag, should be empty if no reason to do otherwise
          BasicConsumeOptions::default(),
          FieldTable::new(),
        )
        .map(move |stream| (publisher_channel, channel, stream))
        .map_err(Error::from)
    })
    .and_then(move |(publisher_channel, channel, stream)| {
      debug!("Stream of message is open, let's consume!");
      let publisher = Arc::new(Mutex::new(publisher_channel));
      stream.map_err(Error::from).for_each(move |delivery| {
        let delivery_tag = delivery.delivery_tag;
        debug!("Received message {}", delivery_tag);
        let publisher = publisher.clone();
        let actions = action.clone();
        let input_message = Message::from(delivery);
        let (tx, rx) = futures::sync::mpsc::channel(0);
        tokio::spawn(
          futures::stream::iter_ok(actions)
            .map(move |action| (action, input_message.clone()))
            .for_each(move |(action, input_message)| {
              let tx = tx.clone();
              Delay::new(Instant::now() + Duration::from_secs(action.schedule.seconds as u64))
                .then(move |_| {
                  let action = action.clone();
                  let input_message = input_message.clone();
                  let mut rng = thread_rng();
                  let evaluator = Random::new(&mut rng);
                  debug!("Generating a message...");
                  handle_message(&action, &input_message, &evaluator)
                })
                .and_then(move |msg| {
                  debug!("We have a message: send it through channel");
                  tx.send(msg).map(|_| ()).map_err(Error::from)
                })
                .map_err(|e| error!("Error: {}", e))
            }),
        );
        let publisher = publisher.clone();
        let channel = channel.clone();
        tokio::spawn(
          rx.map_err(|_| error!("Error on rx stream"))
            .for_each(move |msg| {
              debug!("Received a message to send from the channel");
              let publisher = publisher.lock().unwrap();
              publisher
                .basic_publish(
                  &msg.route.exchange,
                  &msg.route.routing_key,
                  msg.payload,
                  BasicPublishOptions::default(),
                  to_amqp_props(&msg.headers),
                )
                .map(|_| {
                  info!("Message published!!");
                  ()
                })
                .map_err(|e| error!("Publishing a message: {}", e))
            }),
        );
        channel.basic_ack(delivery_tag, false).map_err(Error::from)
      })
    })
}

impl From<Delivery> for Message {
  fn from(delivery: Delivery) -> Message {
    Message {
      payload: delivery.data,
      route: Route {
        exchange: delivery.exchange,
        routing_key: delivery.routing_key,
      },
      headers: to_headers_map(&delivery.properties),
    }
  }
}

fn to_amqp_props(headers: &Headers) -> BasicProperties {
  let (properties, custom_headers) = headers.iter().fold(
    (BasicProperties::default(), FieldTable::new()),
    |(props, mut h), (k, v)| match (&**k, v) {
      ("content_type", Str(s)) => (props.with_content_type(s.clone()), h),
      ("content_encoding", Str(s)) => (props.with_content_encoding(s.clone()), h),
      ("delivery_mode", Int(i)) => (props.with_delivery_mode(*i as u8), h),
      ("priority", Int(i)) => (props.with_priority(*i as u8), h),
      ("correlation_id", Str(s)) => (props.with_correlation_id(s.clone()), h),
      ("reply_to", Str(s)) => (props.with_reply_to(s.clone()), h),
      ("expiration", Str(s)) => (props.with_expiration(s.clone()), h),
      ("message_id", Str(s)) => (props.with_message_id(s.clone()), h),
      ("timestamp", Int(i)) => (props.with_timestamp(*i as u64), h),
      ("type", Str(s)) => (props.with_type_(s.clone()), h),
      ("user_id", Str(s)) => (props.with_user_id(s.clone()), h),
      ("app_id", Str(s)) => (props.with_app_id(s.clone()), h),
      ("cluster_id", Str(s)) => (props.with_cluster_id(s.clone()), h),
      (_, Str(s)) => {
        h.insert(k.clone(), AMQPValue::LongString(s.clone()));
        (props, h)
      }
      (_, Int(i)) => {
        h.insert(k.clone(), AMQPValue::LongLongInt(*i));
        (props, h)
      }
      (_, Real(r)) => {
        h.insert(k.clone(), AMQPValue::Double(*r));
        (props, h)
      }
    },
  );
  properties.with_headers(custom_headers)
}

fn to_hvalue(amqp_value: &AMQPValue) -> Option<HValue> {
  match amqp_value {
    AMQPValue::ShortShortInt(i) => Some(Int(*i as i64)),
    AMQPValue::ShortShortUInt(i) => Some(Int(*i as i64)),
    AMQPValue::ShortInt(i) => Some(Int(*i as i64)),
    AMQPValue::ShortUInt(i) => Some(Int(*i as i64)),
    AMQPValue::LongInt(i) => Some(Int(*i as i64)),
    AMQPValue::LongUInt(i) => Some(Int(*i as i64)),
    AMQPValue::LongLongInt(i) => Some(Int(*i as i64)),
    AMQPValue::LongString(s) => Some(Str(s.clone())),
    AMQPValue::Timestamp(i) => Some(Int(*i as i64)),
    AMQPValue::Float(_) => None,
    AMQPValue::Double(_) => None,
    AMQPValue::DecimalValue(_) => None,
    AMQPValue::FieldArray(_) => None,
    AMQPValue::FieldTable(_) => None,
    AMQPValue::ByteArray(_) => None,
    AMQPValue::Void => None,
    AMQPValue::Boolean(_) => None,
  }
}

fn convert_std_header<T>(
  headers: &mut Headers,
  header_name: &str,
  props: &BasicProperties,
  value_extractor: impl Fn(&BasicProperties) -> &Option<T>,
  hvalue_factory: impl Fn(&T) -> HValue,
) where
  T: Sized,
{
  match value_extractor(props) {
    Some(ref c) => {
      let k = c.clone();
      headers.insert(header_name.to_string(), hvalue_factory(*&k));
      ()
    }
    _ => (),
  }
}

fn to_headers_map(props: &BasicProperties) -> Headers {
  let mut headers = Headers::new();

  convert_std_header(
    &mut headers,
    "content_type",
    props,
    BasicProperties::content_type,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "content_encoding",
    props,
    BasicProperties::content_encoding,
    move |s| Str(s.clone()),
  );
  match props.headers() {
    Some(t) => {
      for (k, v) in t.iter() {
        match to_hvalue(v) {
          Some(hv) => {
            headers.insert(k.clone(), hv);
            ()
          }
          None => (),
        }
      }
      ()
    }
    _ => (),
  };
  convert_std_header(
    &mut headers,
    "delivery_mode",
    props,
    BasicProperties::delivery_mode,
    move |i| Int(*i as i64),
  );
  convert_std_header(
    &mut headers,
    "priority",
    props,
    BasicProperties::priority,
    move |i| Int(*i as i64),
  );
  convert_std_header(
    &mut headers,
    "correlation_id",
    props,
    BasicProperties::correlation_id,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "reply_to",
    props,
    BasicProperties::reply_to,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "expiration",
    props,
    BasicProperties::expiration,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "message_id",
    props,
    BasicProperties::message_id,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "timestamp",
    props,
    BasicProperties::timestamp,
    move |i| Int(*i as i64),
  );
  convert_std_header(
    &mut headers,
    "type",
    props,
    BasicProperties::type_,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "user_id",
    props,
    BasicProperties::user_id,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "app_id",
    props,
    BasicProperties::app_id,
    move |s| Str(s.clone()),
  );
  convert_std_header(
    &mut headers,
    "cluster_id",
    props,
    BasicProperties::cluster_id,
    move |s| Str(s.clone()),
  );
  headers
}

fn create_client(
  amqp_connection: &str,
) -> impl Future<Item = lapin::client::Client<AMQPStream>, Error = Error> {
  trace!("Opening a connection");
  amqp_connection
    .connect_cancellable(|err| {
      error!("heartbeat error: {:?}", err);
    })
    .map_err(Error::from)
    .map(|(client, _)| client)
}

pub fn run() {
  info!("running server");
  let imposter = super::model::imposter::create_stub_imposter(); // TODO: load from config file
  bootstrap(imposter);
}
