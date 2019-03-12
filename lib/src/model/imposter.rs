use failure::Error;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env::var;
use std::iter;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub type Connection = String;

pub type QueueName = String;
pub type ExchangeName = String;
pub type RoutingKey = String;

// MessageHandler :: Delivery -> ()
// look into the map for a matching reactor
// if not found -> ack
// if found -> create a Message from the Delivery, call the reactor for an Action, then pass the action

// TODO: register each consumer, and bind it the message handler
pub struct Imposter {
  pub connection: Connection,
  pub reactors: Vec<ReactorSpec>,
  pub generators: Vec<GeneratorSpec>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReactorSpec {
  // queue is always autodelete
  pub queue: QueueName,
  pub exchange: ExchangeName,
  pub routing_key: RoutingKey,
  pub action: Vec<ActionSpec>,
}

pub type HValue = Lit;
pub type Headers = HashMap<String, HValue>;

#[derive(Clone, Debug, PartialEq)]
pub struct ActionSpec {
  pub to: RouteSpec,
  pub variables: Variables,
  pub payload: PayloadTemplate,
  pub headers: HeadersSpec, // can contain template
  pub schedule: ScheduleSpec,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
  Int(i64),
  Str(String),
  Real(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum VarRef {
  Int(String),
  Str(String),
  Real(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeaderValueSpec {
  Lit(Lit),
  VarRef(VarRef),
}
pub type HeadersSpec = HashMap<String, HeaderValueSpec>;
pub type VariablesSpec = HashMap<String, VarSpec>;
pub type Variables = HashMap<String, Variable>;

#[derive(Clone, Debug, PartialEq)]
pub struct VarSpec {
  always_eval: bool,
  var: Var,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Variable {
  Lit(Lit), // for variables evaluated once and for all
  Var(Var), // for variables to always be evaluated
}

#[derive(Clone, Debug, PartialEq)]
pub enum Var {
  StrJsonPath(String),
  IntJsonPath(String),
  UuidGen,
  StrGen(u8),
  IntGen,
  RealGen,
  Env(String),
  StrHeader(String),
  IntHeader(String),
  DateTime,
  Timestamp,
}

struct Random<'a, T: Rng> {
  rng: RefCell<&'a mut T>,
}

impl<'a, T: Rng> Random<'a, T> {
  fn eval_int(&self, variable: &Var, input_message: &Message) -> Result<i64, Error> {
    match &variable {
      Var::Env(e) => var(e)
        .map_err(Error::from)
        .and_then(|s| s.parse::<i64>().map_err(Error::from)),
      Var::IntGen => Ok(self.rng.borrow_mut().next_u64() as i64),
      Var::IntHeader(h) => Ok(42),   // TODO: get from headers
      Var::IntJsonPath(p) => Ok(42), // TODO: get from message body
      Var::Timestamp => Ok(self.current_time() as i64),
      _ => Err(format_err!("Cannot get an int from {:?}", variable)),
    }
  }

  fn eval_str(&self, variable: &Var, input_message: &Message) -> Result<String, Error> {
    match &variable {
      Var::Env(e) => var(e).map_err(Error::from),
      Var::StrGen(sz) => Ok(self.gen_str(*sz as usize)),
      Var::StrHeader(h) => Ok("to_do".to_owned()), // get from headers
      Var::StrJsonPath(p) => Ok("to_do".to_owned()), // get from body
      Var::DateTime => Ok("to_do".to_owned()),     // current time
      Var::UuidGen => Ok(Uuid::new_v4()),          // TODO
      _ => Err(format_err!("Cannot get a string from {:?}", variable)),
    }
  }

  fn eval_real(&self, variable: &Var, input_message: &Message) -> Result<f64, Error> {
    match &variable {
      Var::Env(e) => Ok(42.0),  // TODO
      Var::RealGen => Ok(42.0), // TODO
      _ => Err(format_err!("Cannot get a real from {:?}", variable)),
    }
  }

  fn gen_str(&self, sz: usize) -> String {
    iter::repeat(())
      .map(|_| self.rng.borrow_mut().sample(Alphanumeric))
      .take(sz)
      .collect()
  }

  fn current_time(&self) -> u64 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 // should not fail
  }
}

// connect(imposter.connection)
// create_reactors(spec) -> Vec<Reactor> = 1 channel + 1 queue / consumer
// create_generators(spec) -> Vec<Generator> = 1 channel / generator to publish (1 tokio task – timer)

pub fn create_stub_imposter() -> Imposter {
  // DELETE when config is loaded from file
  // TODO: add variables spec to be transformed into Variables, which become a simple map of Literal values
  let mut variables = Variables::new();
  variables.insert(
    "uuid".to_owned(),
    Variable::Lit(Lit::Str("12345678-abcdef".to_owned())),
  );
  let mut headers = HeadersSpec::new();
  headers.insert(
    "content_type".to_owned(),
    HeaderValueSpec::Lit(Lit::Str("application/json".to_owned())),
  );
  headers.insert(
    "message_id".to_owned(),
    HeaderValueSpec::VarRef(VarRef::Str("uuid".to_owned())),
  );
  Imposter {
    connection: "amqp://bob:bob@localhost:5672/test?heartbeat=20".to_owned(),
    reactors: vec![ReactorSpec {
      queue: "bob-q".to_owned(),
      exchange: "bob-x".to_owned(),
      routing_key: "r.k.1".to_owned(),
      action: vec![ActionSpec {
        to: RouteSpec {
          exchange: None,
          routing_key: None,
        },
        headers,
        variables,
        payload: "mon id est: {{ uuid }}".to_owned(),
        schedule: ScheduleSpec { seconds: 3 },
      }],
    }],
    generators: vec![GeneratorSpec {
      cron: "".to_owned(),
      action: vec![],
    }],
  }
}
/*
fn use_it() {
  let v = Var::IntGen;
  let msg = Message {
    payload: vec![],
    headers: HashMap::new(),
    route: Route {
      exchange: "".to_owned(),
      routing_key: "".to_owned(),
    },
  };
  let mut rng = thread_rng();
  let r = Random {
    rng: RefCell::new(&mut rng),
  };
  let n = eval::<u64, Eval<u64>>(&r, &msg, &v);
  let num = n.unwrap_or_default();

  println!("{}", num);
}*/

#[derive(Clone, Debug, PartialEq)]
pub struct RouteSpec {
  pub exchange: Option<ExchangeName>,
  pub routing_key: Option<RoutingKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
  pub exchange: ExchangeName,
  pub routing_key: RoutingKey,
}

pub type CronExpr = String;

pub struct GeneratorSpec {
  // TODO: do not include for now
  pub cron: CronExpr,
  pub action: Vec<ActionSpec>,
}

pub type PayloadTemplate = String;

// ************************

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleSpec {
  pub seconds: u8,
}

impl From<ScheduleSpec> for Schedule {
  fn from(spec: ScheduleSpec) -> Schedule {
    match spec.seconds {
      0 => Schedule::Now,
      n => Schedule::Delay(Duration::from_secs(n.into())),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
  pub payload: Vec<u8>,
  pub headers: Headers,
  pub route: Route,
}

// **************************

#[derive(Clone, Debug, PartialEq)]
enum Schedule {
  Now,
  Delay(Duration),
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn schedule() {
    let spec = ScheduleSpec { seconds: 5 };

    let sched: Schedule = spec.into();

    assert_eq!(sched, Schedule::Delay(Duration::from_secs(5)));
  }
}
