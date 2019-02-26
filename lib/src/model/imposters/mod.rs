pub mod action;
pub mod chain;
pub mod function;
pub mod msg;
pub mod reactor;
pub mod tpl;

use self::action::Action;
use self::function::*;
use self::msg::InputMessage;
use self::reactor::ReactorImposter;

pub fn create_logger_reactor() -> impl ReactorImposter {
  FnReactor::new(|input: InputMessage| {
    debug!("{}", input.0);
    Action::DoNothing
  })
}
