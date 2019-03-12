use cron::Schedule;
use failure::{err_msg, Error};
use futures::future::lazy;
use futures::sync::mpsc;
use futures::IntoFuture;
use futures::Sink;
use futures::Stream;
use lapin_futures_rustls::{
  lapin, lapin::channel::*, lapin::message::Delivery, lapin::types::*, AMQPConnectionRustlsExt,
  AMQPStream,
};
use model::imposter::{Lit::*, *};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::iter;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::prelude::Future;
use tokio::timer::Delay;

fn bootstrap(imposter: Imposter) {
  tokio::run(lazy(|| {
    let program = create_client(&imposter.connection).and_then(move |client| {
      futures::stream::iter_ok(imposter.reactors)
        .for_each(move |reactor| tokio::spawn(create_reactor(&client, reactor)))
        .into_future()
        .map_err(|_| err_msg("Couldn't spawn the consumer task"))
        .map(move |_| ())
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
  reactor: ReactorSpec,
) -> impl Future<Item = (), Error = ()> {
  let rk = reactor.routing_key.clone();
  let xchg = reactor.exchange.clone();
  let q = reactor.queue.clone();
  client
    .create_channel()
    .and_then(move |channel| {
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
        .map(move |queue| (channel, queue))
    })
    .and_then(move |(channel, queue)| {
      debug!("Binding {}======{}=====>{}", &queue.name(), &rk, &xchg);
      channel
        .queue_bind(
          &queue.name(),
          &xchg,
          &rk,
          QueueBindOptions::default(),
          FieldTable::new(),
        )
        .map(move |_| (channel, queue))
    })
    .and_then(move |(channel, queue)| {
      debug!("Consuming on {}", &queue.name());
      channel
        .basic_consume(
          &queue,
          "", // consumer tag, should be empty if no reason to do otherwise
          BasicConsumeOptions::default(),
          FieldTable::new(),
        )
        .map(move |stream| (channel, stream))
    })
    .and_then(move |(channel, stream)| {
      debug!("Stream of message is open, let's consume!");
      stream.for_each(move |delivery| {
        debug!("Received message {}", delivery.delivery_tag);
        let (tx, rx) = mpsc::channel::<Message>(reactor.action.len());
        let actions = reactor.action.clone();
        for action in actions {
          let transmitter = tx.clone();
          let d = delivery.clone();
          tokio::spawn(
            delay(action.schedule.seconds as u64, &transmitter, move || {
              debug!("Handling the message...");
              handle_message(&action, &Message::from(d.clone()))
            })
            .map_err(|e| error!("Error handling the message: {}", e)),
          );
        }
        let publish_channel = channel.clone();
        tokio::spawn(rx.for_each(move |m| {
          debug!("Publishing a message");
          publish_channel
            .basic_publish(
              &m.route.exchange,
              &m.route.routing_key,
              m.payload,
              BasicPublishOptions::default(),
              to_amqp_props(&m.headers),
            )
            .map_err(|e| error!("Error publishing a message: {}", e))
            .map(|_| ())
        }));
        channel.basic_ack(delivery.delivery_tag, false)
      })
    })
    .map_err(move |err| error!("got error {}", err))
    .map(|_| ())
}

fn handle_message(action: &ActionSpec, input_message: &Message) -> Result<Message, Error> {
  let payload = action.payload.fill(&input_message, &action.variables)?;
  let headers = action.headers.fill(&input_message, &action.variables)?;
  let route = action.to.fill(&input_message, &action.variables)?;
  Ok(Message {
    headers,
    payload: payload.into_bytes(),
    route,
  })
}

trait Template<R> {
  fn fill(&self, input_message: &Message, vars: &Variables) -> Result<R, Error>;
}

fn get_reply_to(msg: &Message) -> Option<String> {
  msg
    .headers
    .get(&"reply_to".to_owned())
    .and_then(|v| match v {
      Str(x) => Some(x.clone()),
      _ => None, // should never happen
    })
}

// TODO: define what are the best errors
impl Template<Route> for RouteSpec {
  fn fill(&self, input_message: &Message, _vars: &Variables) -> Result<Route, Error> {
    match (&self.exchange, &self.routing_key) {
      (None, _) => Ok(Route {
        exchange: "".to_owned(),
        routing_key: get_reply_to(&input_message).unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), _) if e.is_empty() => Ok(Route {
        exchange: e.clone(),
        routing_key: get_reply_to(&input_message).unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), None) => Ok(Route {
        exchange: e.clone(),
        routing_key: get_reply_to(&input_message).unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), Some(ref r)) if r.is_empty() => Ok(Route {
        exchange: e.clone(),
        routing_key: get_reply_to(&input_message).unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), Some(ref r)) => Ok(Route {
        exchange: e.clone(),
        routing_key: r.clone(),
      }),
    }
  }
}

fn eval_var_ref(
  var_ref: &VarRef,
  input_message: &Message,
  vars: &Variables,
) -> Result<HValue, Error> {
  match var_ref {
    VarRef::Str(r) => match vars.get(r) {
      // TODO call model's functions
      Some(Variable::Lit(Str(s))) => Ok(Str(s.clone())),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(v)) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::StrGen(sz))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::StrHeader(h))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::StrJsonPath(p))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::DateTime)) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Int(ref r) => match vars.get(r) {
      Some(Variable::Lit(Int(i))) => Ok(Int(*i)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(Var::Env(e))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntGen)) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntHeader(h))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntJsonPath(p))) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(Var::Timestamp)) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Real(ref r) => match vars.get(r) {
      Some(Variable::Lit(Real(f))) => Ok(Real(*f)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(Var::RealGen)) => Ok(Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
  }
}

fn eval_spec(
  spec: &HeaderValueSpec,
  input_message: &Message,
  vars: &Variables,
) -> Result<HValue, Error> {
  match spec {
    HeaderValueSpec::Lit(l) => Ok(l.clone()),
    HeaderValueSpec::VarRef(var_ref) => eval_var_ref(var_ref, input_message, vars),
  }
}

impl Template<Headers> for HeadersSpec {
  fn fill(&self, input_message: &Message, vars: &Variables) -> Result<Headers, Error> {
    self
      .iter()
      .fold(Ok(Headers::new()), |acc: Result<Headers, Error>, (k, v)| {
        let h = &mut acc?;
        let value = eval_spec(v, input_message, vars)?;
        h.insert(k.clone(), value);
        Ok(h.clone())
      })
  }
}

impl Template<String> for PayloadTemplate {
  fn fill(&self, input_message: &Message, vars: &Variables) -> Result<String, Error> {
    Ok(String::from("")) // TODO: implement
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

// after a delay of seconds s, produce a value of type T using value_fn, then send it using tx
fn delay<T>(
  seconds: u64,
  tx: &futures::sync::mpsc::Sender<T>,
  value_fn: impl Fn() -> Result<T, Error>,
) -> impl Future<Item = (), Error = Error>
where
  T: Send + Sync + 'static,
{
  let sender = tx.clone();
  Delay::new(Instant::now() + Duration::from_secs(seconds))
    .then(move |_| {
      debug!("Elapsed! Sending a cross-task message over the channel");
      let value = value_fn();
      match value {
        Ok(msg) => {
          sender.send(msg);
          Ok(())
        }
        Err(e) => Err(e),
      }
    })
    .map_err(Error::from)
    .map(|_| ())
}

// run should pass the imposter structure to bootstrap which will be in charge of interpreting it
pub fn run() {
  info!("running server");
  let imposter = super::model::imposter::create_stub_imposter(); // TODO: load from config file
  bootstrap(imposter);
}
