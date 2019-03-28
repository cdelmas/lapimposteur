use super::super::util::read_file;
use chrono::*;
use failure::{err_msg, Error};
use jsonpath::Selector;
use mustache::{compile_str, Data, MapBuilder};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::de::DeserializeOwned;
use serde::Deserialize;
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

#[derive(Debug, PartialEq, Deserialize)]
pub struct Imposter {
  pub connection: Connection,
  pub reactors: Vec<ReactorSpec>,
  #[serde(skip)] // TODO: add later
  pub generators: Vec<GeneratorSpec>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ReactorSpec {
  pub queue: QueueName,
  pub exchange: ExchangeName,
  pub routing_key: RoutingKey,
  pub action: Vec<ActionSpec>,
}

pub type HValue = Lit;

pub type Headers = HashMap<String, HValue>;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ActionSpec {
  pub to: RouteSpec,
  pub variables: VariablesSpec,
  pub payload: PayloadTemplate,
  pub headers: HeadersSpec,
  pub schedule: ScheduleSpec,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Lit {
  Int(i64),
  Str(String),
  Real(f64),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum VarRef {
  Int(String),
  Str(String),
  Real(String),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum HeaderValueSpec {
  Lit(Lit),
  VarRef(VarRef),
}
pub type HeadersSpec = HashMap<String, HeaderValueSpec>;
pub type VariablesSpec = HashMap<String, VarSpec>;
pub type Variables = HashMap<String, Lit>;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VarSpec(pub Var);

impl VarSpec {
  pub fn new(var: Var) -> VarSpec {
    VarSpec { 0: var }
  }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type", content = "param")]
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
  Lit(Lit),
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
        }),
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

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSpec {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub exchange: Option<ExchangeName>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub routing_key: Option<RoutingKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
  pub exchange: ExchangeName,
  pub routing_key: RoutingKey,
}

pub type CronExpr = String;

#[derive(PartialEq, Debug, Deserialize)]
pub struct GeneratorSpec {
  pub cron: CronExpr,
  pub action: Vec<ActionSpec>,
}

#[derive(PartialEq, Clone, Debug, Deserialize)]
pub enum PayloadTemplate {
  Inline(String),
  File(String),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
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
  debug!("Computing variables");
  let variables = eval_var_spec(&action.variables, input_message, evaluator)?;
  trace!("Filling the payload template...");
  let payload = action.payload.fill(&variables)?;
  trace!("Filling the headers template...");
  let headers = action.headers.fill(&variables)?;
  trace!("Filling the route template...");
  let route = action.to.fill(&variables)?;
  Ok(Message {
    headers,
    payload: payload.into_bytes(),
    route,
  })
}

trait Template<T> {
  fn fill(&self, vars: &Variables) -> Result<T, Error>;
}

fn get_reply_to(vars: &Variables) -> Result<String, Error> {
  match vars.get("reply_to") {
    Some(Lit::Str(r)) => Ok(r.clone()),
    Some(_) => Err(err_msg("reply_to has not the good type")),
    None => Err(err_msg("Variable not found: reply_to")),
  }
}

impl Template<Route> for RouteSpec {
  fn fill(&self, vars: &Variables) -> Result<Route, Error> {
    match (&self.exchange, &self.routing_key) {
      (None, _) => {
        let reply_to = get_reply_to(vars)?;
        Ok(Route {
          exchange: "".to_owned(),
          routing_key: reply_to,
        })
      }
      (Some(ref e), _) if e.is_empty() => {
        let reply_to = get_reply_to(vars)?;
        Ok(Route {
          exchange: e.clone(),
          routing_key: reply_to,
        })
      }
      (Some(ref e), None) => {
        let reply_to = get_reply_to(vars)?;
        Ok(Route {
          exchange: e.clone(),
          routing_key: reply_to,
        })
      }
      (Some(ref e), Some(ref r)) if r.is_empty() => {
        let reply_to = get_reply_to(vars)?;
        Ok(Route {
          exchange: e.clone(),
          routing_key: reply_to,
        })
      }
      (Some(ref e), Some(ref r)) => Ok(Route {
        exchange: e.clone(),
        routing_key: r.clone(),
      }),
    }
  }
}

fn eval_var_ref(var_ref: &VarRef, vars: &Variables) -> Result<HValue, Error> {
  match var_ref {
    VarRef::Str(r) => match vars.get(r) {
      Some(Lit::Str(s)) => Ok(Lit::Str(s.clone())),
      Some(_) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Int(ref r) => match vars.get(r) {
      Some(Lit::Int(i)) => Ok(Lit::Int(*i)),
      Some(_) => Err(format_err!("Type mismatch for variable reference {}", &r)),
      None => Err(format_err!("Variable not found {}", &r)),
    },
    VarRef::Real(ref r) => match vars.get(r) {
      Some(Lit::Real(f)) => Ok(Lit::Real(*f)),
      Some(_) => Err(format_err!("Type mismatch for variable reference {}", &r)),
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

fn eval_header_spec(spec: &HeaderValueSpec, vars: &Variables) -> Result<HValue, Error> {
  match spec {
    HeaderValueSpec::Lit(l) => Ok(l.clone()),
    HeaderValueSpec::VarRef(var_ref) => eval_var_ref(var_ref, vars),
  }
}

impl Template<Headers> for HeadersSpec {
  fn fill(&self, vars: &Variables) -> Result<Headers, Error> {
    self
      .iter()
      .fold(Ok(Headers::new()), |acc: Result<Headers, Error>, (k, v)| {
        let h = &mut acc?;
        let value = eval_header_spec(v, vars)?;
        h.insert(k.clone(), value);
        Ok(h.clone())
      })
  }
}

impl Template<String> for PayloadTemplate {
  fn fill(&self, vars: &Variables) -> Result<String, Error> {
    let data = to_hash_map(vars)?;
    let payload = match self {
      PayloadTemplate::Inline(s) => Ok(s.clone()),
      PayloadTemplate::File(p) => read_file(p),
    }?;
    let template = compile_str(&payload)?;
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

fn eval_var_spec<E>(
  var_specs: &VariablesSpec,
  input_message: &Message,
  evaluator: &E,
) -> Result<Variables, Error>
where
  E: Eval<i64> + Eval<String> + Eval<f64>,
{
  let variables = match input_message.get_reply_to() {
    Some(r) => {
      let mut variables = Variables::new();
      variables.insert("reply_to".to_owned(), Lit::Str(r));
      variables
    }
    None => Variables::new(),
  };

  var_specs.iter().fold(Ok(variables), |acc, (k, var_spec)| {
    let mut vars = acc?;
    let spec = var_spec.0.clone();
    match spec {
      Var::Lit(v) => {
        vars.insert(k.clone(), v.clone());
        Ok(vars)
      }
      x @ Var::StrHeader(_) => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::StrJsonPath(_) => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::StrGen(_) => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::Env(_) => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::UuidGen => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::DateTime => {
        let s_val: String = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Str(s_val));
        Ok(vars)
      }
      x @ Var::IntGen => {
        let i_val: i64 = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Int(i_val));
        Ok(vars)
      }
      x @ Var::IntHeader(_) => {
        let i_val: i64 = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Int(i_val));
        Ok(vars)
      }
      x @ Var::IntJsonPath(_) => {
        let i_val: i64 = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Int(i_val));
        Ok(vars)
      }
      x @ Var::Timestamp => {
        let i_val: i64 = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Int(i_val));
        Ok(vars)
      }
      x @ Var::RealGen => {
        let r_val: f64 = evaluator.eval(&x, input_message)?;
        vars.insert(k.clone(), Lit::Real(r_val));
        Ok(vars)
      }
    }
  })
}

fn to_hash_map(vars: &Variables) -> Result<Data, Error> {
  vars
    .iter()
    .fold(Ok(MapBuilder::new()), |acc, (k, v)| {
      let map = acc?;
      match v {
        Lit::Int(i) => map.insert(k.clone(), &i),
        Lit::Str(s) => map.insert(k.clone(), &s.clone()),
        Lit::Real(r) => map.insert(k.clone(), &r),
      }
    })
    .map(MapBuilder::build)
    .map_err(Error::from)
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
