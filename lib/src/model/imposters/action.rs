use super::msg::*;

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
  Composite(Vec<Action>),
}
