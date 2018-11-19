use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct ConnectionInfo {
  pub user: String,
  pub password: String,
  pub vhost: String,
  pub host: String,
  pub port: u16,
  pub tls: bool,
}

impl FromStr for ConnectionInfo {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    use lapin::uri::{AMQPScheme::*, AMQPUri};

    let uri = AMQPUri::from_str(s).map_err(|_| ())?; // swallow the error for now
    Ok(ConnectionInfo {
      user: uri.authority.userinfo.username,
      password: uri.authority.userinfo.password,
      vhost: if uri.vhost == "" {
        "/".to_owned() // avoid the no-vhost bug
      } else {
        uri.vhost
      },
      host: uri.authority.host,
      port: uri.authority.port,
      tls: uri.scheme == AMQPS,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn should_create_a_connection_info_from_string() {
    let expected = ConnectionInfo {
      user: "bob".to_owned(),
      password: "bob".to_owned(),
      vhost: "test".to_owned(),
      host: "localhost".to_owned(),
      port: 5672,
      tls: false,
    };

    let result = ConnectionInfo::from_str("amqp://bob:bob@localhost:5672/test");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
  }

  #[test]
  fn should_give_the_default_values() {
    let expected = ConnectionInfo {
      user: "guest".to_owned(),
      password: "guest".to_owned(),
      vhost: "/".to_owned(),
      host: "localhost".to_owned(),
      port: 5672,
      tls: false,
    };

    let result = ConnectionInfo::from_str("amqp://guest:guest@localhost:5672/");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
  }

}
