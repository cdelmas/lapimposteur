use super::super::model::imposter::*;
use serde_json::*;

// TODO: deserialize a complete imposter => create a function for that

mod tests {

  use super::super::super::model::imposter::*;
  use super::*;

  #[test]
  fn should_deserialize_lit_int() {
    let data = "34";

    let value: Lit = serde_json::from_str(data).unwrap();

    assert_eq!(Lit::Int(34), value);
  }

  #[test]
  fn should_deserialize_lit_real() {
    let data = "4.234";

    let value: Lit = serde_json::from_str(data).unwrap();

    assert_eq!(Lit::Real(4.234), value);
  }

  #[test]
  fn should_deserialize_lit_str() {
    let data = r#""hello""#;

    let value: Lit = serde_json::from_str(data).unwrap();

    assert_eq!(Lit::Str("hello".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_ref_int() {
    let data = r#"{ "Int": "ref" }"#;

    let value: VarRef = serde_json::from_str(data).unwrap();

    assert_eq!(VarRef::Int("ref".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_ref_str() {
    let data = r#"{ "Str": "ref" }"#;

    let value: VarRef = serde_json::from_str(data).unwrap();

    assert_eq!(VarRef::Str("ref".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_ref_real() {
    let data = r#"{ "Real": "ref" }"#;

    let value: VarRef = serde_json::from_str(data).unwrap();

    assert_eq!(VarRef::Real("ref".to_owned()), value);
  }

  #[test]
  fn should_deserialize_header_value_spec_lit() {
    let data = r#"
      { "Lit": 42 }
    "#;

    let value: HeaderValueSpec = serde_json::from_str(data).unwrap();

    assert_eq!(HeaderValueSpec::Lit(Lit::Int(42)), value);
  }

  #[test]
  fn should_deserialize_header_value_spec_var_ref() {
    let data = r#"
      { "VarRef": { "Int" : "ref"} }
    "#;

    let value: HeaderValueSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      HeaderValueSpec::VarRef(VarRef::Int("ref".to_owned())),
      value
    );
  }

  #[test]
  fn should_deserialize_var_str_json_path() {
    let data = r#"
      { "type":"StrJsonPath", "param": "json.path" }
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::StrJsonPath("json.path".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_int_json_path() {
    let data = r#"
      { "type":"IntJsonPath", "param": "json.path" }
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::IntJsonPath("json.path".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_uuid_gen() {
    let data = r#"
      {"type":"UuidGen"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::UuidGen, value);
  }

  #[test]
  fn should_deserialize_var_str_gen() {
    let data = r#"
      { "type":"StrGen", "param": 16 }
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::StrGen(16), value);
  }

  #[test]
  fn should_deserialize_var_int_gen() {
    let data = r#"
      {"type":"IntGen"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::IntGen, value);
  }

  #[test]
  fn should_deserialize_var_real_gen() {
    let data = r#"
      {"type":"RealGen"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::RealGen, value);
  }

  #[test]
  fn should_deserialize_var_env() {
    let data = r#"
      {"type":"Env", "param": "HOST"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::Env("HOST".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_str_header() {
    let data = r#"
      {"type":"StrHeader", "param": "toto"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::StrHeader("toto".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_int_header() {
    let data = r#"
      {"type":"IntHeader", "param": "priority"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::IntHeader("priority".to_owned()), value);
  }

  #[test]
  fn should_deserialize_var_date_time() {
    let data = r#"
      {"type":"DateTime"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::DateTime, value);
  }

  #[test]
  fn should_deserialize_var_timestamp() {
    let data = r#"
      {"type":"Timestamp"}
    "#;

    let value: Var = serde_json::from_str(data).unwrap();

    assert_eq!(Var::Timestamp, value);
  }

  #[test]
  fn should_deserialize_var_spec() {
    let data = r#"
      {"type":"Timestamp"}
    "#;

    let value: VarSpec = serde_json::from_str(data).unwrap();

    assert_eq!(VarSpec(Var::Timestamp), value);
  }

  #[test]
  fn should_deserialize_an_empty_route_spec() {
    let data = r#"
      {}
    "#;

    let value: RouteSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      RouteSpec {
        exchange: None,
        routing_key: None,
      },
      value
    );
  }

  #[test]
  fn should_deserialize_a_route_spec_with_only_exchange() {
    let data = r#"
      { "exchange": "x-x" }
    "#;

    let value: RouteSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      RouteSpec {
        exchange: Some("x-x".to_owned()),
        routing_key: None,
      },
      value
    );
  }

  #[test]
  fn should_deserialize_a_route_spec_with_only_routing_key() {
    let data = r#"
      { "routingKey": "x.x" }
    "#;

    let value: RouteSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      RouteSpec {
        exchange: None,
        routing_key: Some("x.x".to_owned()),
      },
      value
    );
  }

  #[test]
  fn should_deserialize_schedule_spec() {
    let data = r#"
      { "seconds": 8 }
    "#;

    let value: ScheduleSpec = serde_json::from_str(data).unwrap();

    assert_eq!(ScheduleSpec { seconds: 8 }, value);
  }

  #[test]
  fn should_deserialize_action_spec() {
    let data = r#"
      { 
        "to": { "exchange": "x", "routingKey": "r.k" },
        "variables": {
          "k": { "type": "UuidGen" },
          "input.data.id": {"type":"StrJsonPath", "param": "input.path"}
        },
        "payload": "{ \"value\": {{ k }} }",
        "headers": {
          "header.str": { "VarRef": { "Str": "input.data.id" } }
        },
        "schedule": { "seconds": 3 }
      }
    "#;

    let value: ActionSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      ActionSpec {
        to: RouteSpec {
          exchange: Some("x".to_owned()),
          routing_key: Some("r.k".to_owned())
        },
        variables: hashmap! {
          "k".to_owned() => VarSpec::new(Var::UuidGen),
          "input.data.id".to_owned() => VarSpec::new(Var::StrJsonPath("input.path".to_owned()))
        },
        payload: "{ \"value\": {{ k }} }".to_owned(),
        headers: hashmap! { "header.str".to_owned() => HeaderValueSpec::VarRef(VarRef::Str("input.data.id".to_owned())) },
        schedule: ScheduleSpec { seconds: 3 },
      },
      value
    );
  }

  #[test]
  fn should_deserialize_a_reactor_spec() {
    let data = r#"
      {
        "queue": "a-queue",
        "routing_key": "a.routing.key",
        "exchange": "an.exchange",
        "action": [
          { 
            "to": { "exchange": "x", "routingKey": "r.k" },
            "variables": {},
            "payload": "Hello",
            "headers": {
              "content_type": {"Lit": "application/json"}
            },
            "schedule": { "seconds": 0 }
          }
        ]
      }
    "#;

    let value: ReactorSpec = serde_json::from_str(data).unwrap();

    assert_eq!(
      ReactorSpec {
        queue: "a-queue".to_owned(),
        exchange: "an.exchange".to_owned(),
        routing_key: "a.routing.key".to_owned(),
        action: vec![ActionSpec {
          to: RouteSpec {
            exchange: Some("x".to_owned()),
            routing_key: Some("r.k".to_owned())
          },
          variables: hashmap! {},
          payload: "Hello".to_owned(),
          headers: hashmap! { "content_type".to_owned() => HeaderValueSpec::Lit(Lit::Str("application/json".to_owned())) },
          schedule: ScheduleSpec { seconds: 0 },
        }]
      },
      value
    );
  }

  #[test]
  fn should_deserialize_a_do_nothing_imposter() {
    let data = r#"
      {
        "connection": "amqps://guest:guest@localhost:5672/test",
        "reactors": []
      }
    "#;

    let value: Imposter = serde_json::from_str(data).unwrap();

    assert_eq!(
      Imposter {
        connection: "amqps://guest:guest@localhost:5672/test".to_owned(),
        reactors: vec![],
        generators: vec![],
      },
      value
    );
  }
}
