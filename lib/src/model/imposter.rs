use std::collections::HashMap;

pub struct InputMessage<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Message {
  pub body: Body,
  pub headers: Headers,
}

impl Message {
  pub fn new(body: Body, headers: Headers) -> Self {
    Message { body, headers }
  }
}

pub trait ReactorImposter {
  fn react(&self, InputMessage) -> Action;
}

pub mod imposters {
  use super::*;
  pub struct LoggerReactor;
  impl ReactorImposter for LoggerReactor {
    fn react(&self, input: InputMessage) -> Action {
      debug!("{}", input.0);
      Action::DoNothing
    }
  }
  pub struct PingPongReactor;
  impl ReactorImposter for PingPongReactor {
    fn react(&self, input: InputMessage) -> Action {
      Action::SendMsg(MessageDispatch {
        message: Message::new(Body(String::from(input.0)), Headers::empty()),
        delay: Schedule::Now,
      })
    }
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

    let result = ReactorImposter::new(move |_| DoNothing).react(InputMessage(&input));

    assert_eq!(DoNothing, result);
  }

}
