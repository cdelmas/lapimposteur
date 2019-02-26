use super::action::*;
use super::msg::*;
use super::reactor::*;

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
