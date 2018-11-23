pub struct InputMessage<'a>(pub &'a str);

#[derive(Debug, PartialEq)]
pub struct Message;

#[derive(Debug, PartialEq)]
pub struct MessageDispatch {
  message: Message,
  delay: Schedule,
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

pub struct ReactorImposter<F>
where
  F: Fn(InputMessage) -> Action,
{
  f: Box<F>,
}

impl<F> ReactorImposter<F>
where
  F: Fn(InputMessage) -> Action,
{
  pub fn new(f: F) -> Self {
    Self { f: Box::new(f) }
  }

  pub fn react(&self, input: InputMessage) -> Action {
    (self.f)(input)
  }
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
