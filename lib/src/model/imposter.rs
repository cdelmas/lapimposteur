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

pub trait ReactorImposter {
  fn react(&self, InputMessage) -> Action;
}

pub mod imposters {
  use super::*;

  pub fn create_logger_reactor() -> impl ReactorImposter {
    FnReactor::new(|input: InputMessage| {
      debug!("{}", input.0);
      Action::DoNothing
    })
  }

  pub struct FnReactor<F>
  where
    F: Fn(InputMessage) -> Action,
  {
    f: Box<F>,
  }
  impl<F> ReactorImposter for FnReactor<F>
  where
    F: Fn(InputMessage) -> Action,
  {
    fn react(&self, input: InputMessage) -> Action {
      (self.f)(input)
    }
  }
  impl<F> FnReactor<F>
  where
    F: Fn(InputMessage) -> Action,
  {
    pub fn new(f: F) -> Self {
      FnReactor { f: Box::new(f) }
    }
  }
}

pub struct ImposterChain<'a> {
  chain: Vec<&'a ReactorImposter>,
}

impl<'a> ImposterChain<'a> {
  pub fn new(imposters: &[&'a ReactorImposter]) -> Self {
    let chain = imposters.to_vec();
    ImposterChain { chain }
  }

  pub fn run(&self, input: InputMessage) -> Option<Action> {
    Some(self.chain[0].react(input))
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

#[derive(Debug, PartialEq)]
pub struct MessageDispatch {
  pub message: Message,
  pub delay: Schedule,
}

#[derive(Debug, PartialEq)]
pub enum Schedule {
  Now,
}

#[derive(Debug, PartialEq)]
pub enum Action {
  DoNothing,
  SendMsg(MessageDispatch),
  SendMsgSeq(Vec<MessageDispatch>),
}

// TODO: GeneratorImposter

#[cfg(test)]
mod tests {

  use super::Action::*;
  use super::*;

  #[test]
  fn should_swallow_input_and_return_nothing() {
    let input = String::from("{}");
    struct Anonymous;
    impl ReactorImposter for Anonymous {
      fn react(&self, _: InputMessage) -> Action {
        DoNothing
      }
    }
    let anonymous_reactor_imposter = Anonymous;

    let result = anonymous_reactor_imposter.react(InputMessage(&input));

    assert_eq!(DoNothing, result);
  }

  #[test]
  fn should_run_the_first_of_the_chain() {
    let reactor1 = imposters::create_logger_reactor();
    let reactor2 = imposters::FnReactor::new(move |_| {
      SendMsg(MessageDispatch {
        message: Message::new(
          Route::new("ex", "rk"),
          Body(String::from("nuclear bom")),
          Headers::empty(),
        ),
        delay: Schedule::Now,
      })
    });
    let chain = ImposterChain::new(&[&reactor1, &reactor2]);

    let result = chain.run(InputMessage("a message"));

    assert!(result.is_some());
    assert_eq!(DoNothing, result.unwrap());
  }
}
