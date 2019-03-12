use chrono::*;
use failure::Error;
use jsonpath::Selector;
use mustache::{compile_str, Data, MapBuilder};
use rand::distributions::Alphanumeric;
use rand::Rng;
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

pub struct Random<'a, R: Rng> {
  rng: RefCell<&'a mut R>,
}

impl<'a, R: Rng> Random<'a, R> {
  pub fn new(rng: &'a mut R) -> Self {
    Random {
      rng: RefCell::new(rng),
    }
  }
}

pub trait Eval<T> {
  fn eval(&self, variable: &Var, input_message: &Message) -> Result<T, Error>;
}

impl<'a, R: Rng> Eval<i64> for Random<'a, R> {
  fn eval(&self, variable: &Var, input_message: &Message) -> Result<i64, Error> {
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
          Lit::Int(i) => Ok(*i),
          _ => Err(format_err!("Cannot get header {} of type Int", h)),
        }),
      Var::IntJsonPath(p) => get_value_from_body(input_message, p),
      Var::Timestamp => Ok(current_time()),
      _ => Err(format_err!("Cannot get an int from {:?}", variable)),
    }
  }
}

impl<'a, R: Rng> Eval<String> for Random<'a, R> {
  fn eval(&self, variable: &Var, input_message: &Message) -> Result<String, Error> {
    match &variable {
      Var::Env(e) => var(e).map_err(Error::from),
      Var::StrGen(sz) => Ok(self.gen_str(*sz as usize)),
      Var::StrHeader(h) => input_message
        .headers
        .get(h)
        .ok_or(format_err!("Cannot get header {}", h))
        .and_then(|l| match l {
          Lit::Str(s) => Ok(s.clone()),
          _ => Err(format_err!("Cannot get header {} of type Str", h)),
        }), // get from headers
      Var::StrJsonPath(p) => get_value_from_body(input_message, p),
      Var::DateTime => Ok(now().to_string()),
      Var::UuidGen => Ok(Uuid::new_v4().to_hyphenated().to_string()),
      _ => Err(format_err!("Cannot get a string from {:?}", variable)),
    }
  }
}

impl<'a, R: Rng> Eval<f64> for Random<'a, R> {
  fn eval(&self, variable: &Var, _input_message: &Message) -> Result<f64, Error> {
    match &variable {
      Var::Env(e) => var(e)
        .map_err(Error::from)
        .and_then(|s| s.parse::<f64>().map_err(Error::from)),
      Var::RealGen => Ok(self.rng.borrow_mut().gen()),
      _ => Err(format_err!("Cannot get a real from {:?}", variable)),
    }
  }
}

impl<'a, R: Rng> Random<'a, R> {
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
    reactors: vec![
      ReactorSpec {
        queue: "bob-q-1".to_owned(),
        exchange: "bob-x".to_owned(),
        routing_key: "r.k.1".to_owned(),
        action: vec![ActionSpec {
          to: RouteSpec {
            exchange: Some("bob-x".to_owned()),
            routing_key: None,
          },
          headers,
          variables,
          payload: "mon id est: {{ uuid }}".to_owned(),
          schedule: ScheduleSpec { seconds: 3 },
        }],
      },
      ReactorSpec {
        queue: "bob-q-2".to_owned(),
        exchange: "bob-x".to_owned(),
        routing_key: "r.k.1".to_owned(),
        action: vec![ActionSpec {
          to: RouteSpec {
            exchange: Some("bob-x".to_owned()),
            routing_key: None,
          },
          headers: HeadersSpec::new(),
          variables: Variables::new(),
          payload: "Message en dur".to_owned(),
          schedule: ScheduleSpec { seconds: 0 },
        }],
      },
    ],
    generators: vec![GeneratorSpec {
      cron: "".to_owned(),
      action: vec![],
    }],
  }
}

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

impl Message {
  fn get_reply_to(&self) -> Option<String> {
    self
      .headers
      .get(&"reply_to".to_owned())
      .and_then(|v| match v {
        Lit::Str(x) => Some(x.clone()),
        _ => None, // should never happen
      })
  }
}

pub fn handle_message<E>(
  action: &ActionSpec,
  input_message: &Message,
  evaluator: &E,
) -> Result<Message, Error>
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  trace!("Filling the payload template...");
  let payload = action
    .payload
    .fill(&input_message, &action.variables, evaluator)?;
  trace!("Filling the headers template...");
  let headers = action
    .headers
    .fill(&input_message, &action.variables, evaluator)?;
  trace!("Filling the route template...");
  let route = action
    .to
    .fill(&input_message, &action.variables, evaluator)?;
  Ok(Message {
    headers,
    payload: payload.into_bytes(),
    route,
  })
}

trait Template<T, E> {
  fn fill(&self, input_message: &Message, vars: &Variables, evaluator: &E) -> Result<T, Error>
  where
    E: Eval<i64> + Eval<String> + Eval<f64>;
}

impl<E> Template<Route, E> for RouteSpec {
  fn fill(&self, input_message: &Message, _vars: &Variables, _evaluator: &E) -> Result<Route, Error>
  where
    E: Eval<i64> + Eval<String> + Eval<f64>,
  {
    match (&self.exchange, &self.routing_key) {
      (None, _) => Ok(Route {
        exchange: "".to_owned(),
        routing_key: input_message
          .get_reply_to()
          .unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), _) if e.is_empty() => Ok(Route {
        exchange: e.clone(),
        routing_key: input_message
          .get_reply_to()
          .unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), None) => Ok(Route {
        exchange: e.clone(),
        routing_key: input_message
          .get_reply_to()
          .unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), Some(ref r)) if r.is_empty() => Ok(Route {
        exchange: e.clone(),
        routing_key: input_message
          .get_reply_to()
          .unwrap_or("invalid.key".to_owned()),
      }),
      (Some(ref e), Some(ref r)) => Ok(Route {
        exchange: e.clone(),
        routing_key: r.clone(),
      }),
    }
  }
}

fn eval_var_ref<E>(
  var_ref: &VarRef,
  input_message: &Message,
  vars: &Variables,
  evaluator: &E,
) -> Result<HValue, Error>
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  match var_ref {
    VarRef::Str(r) => match vars.get(r) {
      Some(Variable::Lit(Lit::Str(s))) => Ok(Lit::Str(s.clone())),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(v)) => evaluator.eval(v, input_message).map(Lit::Str),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Int(ref r) => match vars.get(r) {
      Some(Variable::Lit(Lit::Int(i))) => Ok(Lit::Int(*i)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(v)) => evaluator.eval(v, input_message).map(Lit::Int),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Real(ref r) => match vars.get(r) {
      Some(Variable::Lit(Lit::Real(f))) => Ok(Lit::Real(*f)),
      Some(Variable::Lit(_)) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      Some(Variable::Var(v)) => evaluator.eval(v, input_message).map(Lit::Real),
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

fn eval_spec<E>(
  spec: &HeaderValueSpec,
  input_message: &Message,
  vars: &Variables,
  evaluator: &E,
) -> Result<HValue, Error>
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  match spec {
    HeaderValueSpec::Lit(l) => Ok(l.clone()),
    HeaderValueSpec::VarRef(var_ref) => eval_var_ref(var_ref, input_message, vars, evaluator),
  }
}

impl<E> Template<Headers, E> for HeadersSpec
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  fn fill(
    &self,
    input_message: &Message,
    vars: &Variables,
    evaluator: &E,
  ) -> Result<Headers, Error> {
    self
      .iter()
      .fold(Ok(Headers::new()), |acc: Result<Headers, Error>, (k, v)| {
        let h = &mut acc?;
        let value = eval_spec(v, input_message, vars, evaluator)?;
        h.insert(k.clone(), value);
        Ok(h.clone())
      })
  }
}

impl<E> Template<String, E> for PayloadTemplate
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  fn fill(
    &self,
    input_message: &Message,
    vars: &Variables,
    evaluator: &E,
  ) -> Result<String, Error> {
    let data = to_hash_map(vars, input_message, evaluator)?;
    let template = compile_str(self)?;
    let mut out = Cursor::new(Vec::new());
    template.render_data(&mut out, &data)?;
    String::from_utf8(out.into_inner()).map_err(Error::from)
  }
}

impl From<Lit> for String {
  fn from(lit: Lit) -> String {
    match lit {
      Lit::Str(s) => s.clone(),
      Lit::Int(i) => i.to_string(),
      Lit::Real(r) => r.to_string(),
    }
  }
}

fn to_hash_map<E>(vars: &Variables, input_message: &Message, evaluator: &E) -> Result<Data, Error>
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  vars
    .iter()
    .fold(Ok(MapBuilder::new()), |acc, (k, v)| {
      let map = acc?;
      match v {
        Variable::Lit(x) => map
          .insert(k.clone(), &String::from(x.clone()))
          .map_err(Error::from),
        Variable::Var(v) => match v {
          x @ Var::StrHeader(_) => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::StrJsonPath(_) => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::StrGen(_) => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::Env(_) => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::UuidGen => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::DateTime => {
            let s_val: String = evaluator.eval(x, input_message)?;
            Ok(map.insert_str(k.clone(), s_val.clone()))
          }
          x @ Var::IntGen => {
            let i_val: i64 = evaluator.eval(x, input_message)?;
            map.insert(k.clone(), &i_val).map_err(Error::from)
          }
          x @ Var::IntHeader(_) => {
            let i_val: i64 = evaluator.eval(x, input_message)?;
            map.insert(k.clone(), &i_val).map_err(Error::from)
          }
          x @ Var::IntJsonPath(_) => {
            let i_val: i64 = evaluator.eval(x, input_message)?;
            map.insert(k.clone(), &i_val).map_err(Error::from)
          }
          x @ Var::Timestamp => {
            let i_val: i64 = evaluator.eval(x, input_message)?;
            map.insert(k.clone(), &i_val).map_err(Error::from)
          }
          x @ Var::RealGen => {
            let r_val: f64 = evaluator.eval(x, input_message)?;
            map.insert(k.clone(), &r_val).map_err(Error::from)
          }
        },
      }
    })
    .map(MapBuilder::build)
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
