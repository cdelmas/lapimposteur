use super::action::Action;
use super::msg::*;
use super::reactor::*;

pub struct ImposterChain<'a> {
  chain: Vec<&'a ReactorImposter>,
}

impl<'a> ImposterChain<'a> {
  pub fn new(imposters: &[&'a ReactorImposter]) -> Self {
    let chain = imposters.to_vec();
    ImposterChain { chain }
  }

  pub fn run(&self, input: InputMessage) -> Action {
    self.chain[0].react(input) // TODO, should return Composite(chain.map(x.react))
  }
}

#[cfg(test)]
mod tests {
  use super::super::action::{Action::*, MessageDispatch, Schedule};
  use super::super::chain::*;
  use super::super::create_logger_reactor;
  use super::super::function::*;

  #[test]
  fn should_run_the_first_of_the_chain() {
    let reactor1 = create_logger_reactor();
    let reactor2 = FnReactor::new(move |_| {
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

    assert_eq!(DoNothing, result);
  }
}
