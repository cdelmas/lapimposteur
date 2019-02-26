use std::collections::HashMap;

pub struct InputMessage<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Route {
  pub exchange: String,
  pub routing_key: String,
}

impl Route {
  pub fn new(exchange: &str, routing_key: &str) -> Self {
    Route {
      exchange: String::from(exchange),
      routing_key: String::from(routing_key),
    }
  }
}

#[derive(Debug, PartialEq)]
pub struct Message {
  pub route: Route,
  pub body: Body,
  pub headers: Headers,
}

impl Message {
  pub fn new(route: Route, body: Body, headers: Headers) -> Self {
    Message {
      route,
      body,
      headers,
    }
  }
}

#[derive(Debug, PartialEq)]
pub struct Body(pub String);

#[derive(Debug, PartialEq)]
pub struct Headers(pub HashMap<String, String>);

impl Headers {
  pub fn empty() -> Self {
    Headers(HashMap::new())
  }
}