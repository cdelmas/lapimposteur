use super::action::*;
use super::msg::*;
use super::reactor::*;

pub struct TemplateResponseReactor {
  // variables, response template
}

impl TemplateResponseReactor {
  // TODO: create it
}

impl ReactorImposter for TemplateResponseReactor {
  fn react(&self, input: InputMessage) -> Action {
    Action::DoNothing
  }
}
