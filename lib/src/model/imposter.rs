use chrono::*;
use failure::Error;
use jsonpath::Selector;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng, ThreadRng};
use rustache::{HashBuilder, Render};
use serde::de::DeserializeOwned;
use serde_json::{from_value, Value as JsonValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env::var;
use std::io::Cursor;
use std::iter;
use std::time::Duration;
use uuid::Uuid;

pub type Connection = String;

pub type QueueName = String;
pub type ExchangeName = String;
pub type RoutingKey = String;

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
  rng: RefCell<&'a mut T>, // TODO: Arc?
}

impl<'a, T: Rng> Random<'a, T> {
  fn new(rng: &'a mut T) -> Self {
    Random {
      rng: RefCell::new(rng),
    }
  }
}

impl<'a, T: Rng> Random<'a, T> {
  pub fn eval_int(&self, variable: &Var, input_message: &Message) -> Result<i64, Error> {
    match &variable {
      Var::Env(e) => var(e)
        .map_err(Error::from)
        .and_then(|s| s.parse::<i64>().map_err(Error::from)),
      Var::IntGen => Ok(self.rng.borrow_mut().next_u64() as i64),
      Var::IntHeader(h) => input_message
        .headers
        .get(h)
        .ok_or(format_err!("Cannot get header {}", h))
        .and_then(|l| match l {
          Lit::Int(i) => Ok(*i), // TODO eval variables
          _ => Err(format_err!("Cannot get header {} of type Int", h)),
        }),
      Var::IntJsonPath(p) => Ok(42), // TODO: get from message body
      Var::Timestamp => Ok(current_time()),
      _ => Err(format_err!("Cannot get an int from {:?}", variable)),
    }
  }

  pub fn eval_str(&self, variable: &Var, input_message: &Message) -> Result<String, Error> {
    match &variable {
      Var::Env(e) => var(e).map_err(Error::from),
      Var::StrGen(sz) => Ok(self.gen_str(*sz as usize)),
      Var::StrHeader(h) => Ok("to_do".to_owned()), // get from headers
      Var::StrJsonPath(p) => Ok("to_do".to_owned()), // get from body
      Var::DateTime => Ok(now().to_string()),
      Var::UuidGen => Ok(Uuid::new_v4().to_hyphenated().to_string()),
      _ => Err(format_err!("Cannot get a string from {:?}", variable)),
    }
  }

  pub fn eval_real(&self, variable: &Var, input_message: &Message) -> Result<f64, Error> {
    match &variable {
      Var::Env(e) => var(e)
        .map_err(Error::from)
        .and_then(|s| s.parse::<f64>().map_err(Error::from)),
      Var::RealGen => Ok(self.rng.borrow_mut().gen()),
      _ => Err(format_err!("Cannot get a real from {:?}", variable)),
    }
  }

  fn gen_str(&self, sz: usize) -> String {
    iter::repeat(())
      .map(|_| self.rng.borrow_mut().sample(Alphanumeric))
      .take(sz)
      .collect()
  }
}

fn now() -> DateTime<Utc> {
  Utc::now()
}

fn current_time() -> i64 {
  now().timestamp_nanos()
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

pub fn handle_message(action: &ActionSpec, input_message: &Message) -> Result<Message, Error> {
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

// TODO: impl Message
fn get_reply_to(msg: &Message) -> Option<String> {
  msg
    .headers
    .get(&"reply_to".to_owned())
    .and_then(|v| match v {
      Lit::Str(x) => Some(x.clone()),
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
      Some(Variable::Lit(Lit::Str(s))) => Ok(Lit::Str(s.clone())),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(Var::StrGen(sz))) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(Var::StrHeader(h))) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(Var::StrJsonPath(p))) => {
        get_value_from_body::<String>(input_message, p).map(Lit::Str)
      }
      Some(Variable::Var(Var::DateTime)) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Int(ref r) => match vars.get(r) {
      Some(Variable::Lit(Lit::Int(i))) => Ok(Lit::Int(*i)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(Var::Env(e))) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntGen)) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntHeader(h))) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(Var::IntJsonPath(p))) => {
        get_value_from_body::<i64>(input_message, p).map(Lit::Int)
      }
      Some(Variable::Var(Var::Timestamp)) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Real(ref r) => match vars.get(r) {
      Some(Variable::Lit(Lit::Real(f))) => Ok(Lit::Real(*f)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(Var::RealGen)) => Ok(Lit::Str("to_do".to_owned())),
      Some(Variable::Var(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
  }
}

fn get_value_from_body<T>(msg: &Message, json_path: &str) -> Result<T, Error>
where
  T: DeserializeOwned,
{
  let json: JsonValue = serde_json::from_slice(&msg.payload).unwrap();
  let selector = Selector::new(json_path).unwrap();

  let data: Vec<&JsonValue> = selector.find(&json).collect();
  if data.len() == 1 {
    from_value::<T>(data[0].clone()).map_err(Error::from)
  } else {
    Err(format_err!("Cannot get value from path {}", json_path))
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
