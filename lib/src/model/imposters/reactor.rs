use super::action::*;
use super::msg::*;

pub trait ReactorImposter {
  fn react(&self, InputMessage) -> Action;
}

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
}
