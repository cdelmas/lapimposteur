#[cfg(ignored)]
mod tests {

  use jsonpath::Selector;
  use model::imposters::{msg::*};
  use rustache::{HashBuilder, Render};
  use serde_json::Value as JsonValue;
  use std::io::Cursor;

  #[test]
  fn should_find_something_into_the_body() {
    let msg = Message::new(
      Route::new("ex", "rk"),
      Body(String::from(
        r#"
          { 
            "key": 1234,
            "ex": [
              {
                "data": "123"
              }, 
              {
                "data": "456"
              }
            ]
          }
          "#,
      )),
      Headers::empty(),
    );

    let json: JsonValue = serde_json::from_str(&msg.body.0).unwrap();
    let selector = Selector::new("$.ex[0]").unwrap();

    let data: Vec<String> = selector.find(&json).map(|t| t.to_string()).collect();

    assert_eq!(data, vec!["{\"data\":\"123\"}"]);
  }

  #[test]
  fn rustache() {
    let data = HashBuilder::new().insert("hello", "your name");
    let mut out = Cursor::new(Vec::new());
    data.render("{{ hello }}", &mut out).unwrap();
    assert_eq!("your name", String::from_utf8(out.into_inner()).unwrap());
  }

  use nom::*;
  use nom::types::CompleteStr;
  use std::collections::HashMap;

  // NOTE: have a variables file, listing key value pairs
  // have a json template with mustache variables
  // parse the variables file to a hashmap, which is the input of rustache to fill the template

  // variables file
  // variable_name = value // any char follow by chars, digits or _, then spaces or not, then =, then spaces or not, then value
  // value is either : a literal (parsed as a string)
  //       or : a json path
  //       or : a property reference
  //       or : an environment variable reference
  //       or : a generator

  named!(var_map<CompleteStr, HashMap<&str, Value> >,
    dbg_dmp!(map!(many0!(var_decl), |v:Vec<(&str,Value)>| v.into_iter().collect()))
  );

  #[test]
  fn parse_var_file() {
    let res = var_map("id = $.id\nhost = env[HOST]\ncorrId = prop[correlation_id]\nrandomStr = <str>\nnumi = 3\nnumr = 3.2\nstr = a string\n".into());
    assert_eq!(
      res, 
      Ok(("".into(), hashmap!( 
        "id" => Value::Variable(Variable::JsonPath{path: "$.id".to_owned()}),
        "host" => Value::Variable(Variable::Env{name: "HOST".to_owned()}),
        "corrId" => Value::Variable(Variable::Property{name: "correlation_id".to_owned()}),
        "randomStr" => Value::Variable(Variable::Generator{g_type: GeneratorType::Str}),
        "numi" => Value::Literal("3".to_owned()),
        "numr" => Value::Literal("3.2".to_owned()),
        "str" => Value::Literal("a string".to_owned())
      )))
    );
  }

  named!(var_decl<CompleteStr,(&str,Value)>,
    do_parse!(
      opt!(space) >>
      k: var_name >>
      ws!(tag!("=")) >> 
      v: alt!(json_path | prop | env | generator | literal) >>
      opt!(space) >>
      eol >>
      (k.0,v)
    )
  );

 #[test]
  fn parse_json_path_var_decl() {
    let res = var_decl("id = $.id\n".into());
    assert_eq!(res, Ok(("".into(), ("id", Value::Variable(Variable::JsonPath{path: "$.id".to_owned()})))));
  }

  #[test]
  fn parse_literal_var_decl() {
    let res = var_decl("str = a string\n".into());
    assert_eq!(res, Ok(("".into(), ("str", Value::Literal("a string".to_owned())))));
  }
 
  #[derive(PartialEq, Debug)]
  enum Value {
    Literal(Literal),
    Variable(Variable),
  }

  type Literal = String;

  #[derive(PartialEq, Debug)]
  enum Variable {
    JsonPath { path: String, },
    Property { name: String, },
    Env { name: String },
    Generator {
      g_type: GeneratorType,
    },
  }

  // Utils
  named!(literal<CompleteStr, Value>, 
    do_parse!(
      s: take_till!(is_end_of_line) >>
         (Value::Literal(String::from(s.0)))
    )
  );

  #[test]
  fn parse_string_literal() {
    let res = literal("a string \"characters\" {=with weird thing%}".into());

    assert_eq!(res, Ok(("".into(), Value::Literal("a string \"characters\" {=with weird thing%}".to_owned()))));
  }

  #[test]
  fn parse_int_literal() {
    let res = literal("45321".into());

    assert_eq!(res, Ok(("".into(), Value::Literal("45321".to_owned()))));
  }

  #[test]
  fn parse_real_literal() {
    let res = literal("45321.432".into());

    assert_eq!(res, Ok(("".into(), Value::Literal("45321.432".to_owned()))));
  }

  named!(var_name<CompleteStr, CompleteStr>, take_while!(is_var_name_char));

  fn is_end_of_line(c: char) -> bool {
    c == '\n'
  }

  #[test]
  fn parse_var_name(){
    let res = var_name("id0  = ".into());
    
    assert_eq!(res, Ok(("  = ".into(), "id0".into())));
  }

  fn is_underscore(c: char) -> bool {
    c == '_'
  }

  fn is_end_of_value(c: char) -> bool {
    c.is_whitespace() || is_end_of_line(c)
  }

  fn is_var_name_char(c: char) -> bool {
    is_underscore(c) || c.is_alphanumeric()
  }

  fn is_env_name_char(c: char) -> bool {
    is_underscore(c) || c.is_uppercase()
  }

  fn is_prop_name_char(c: char) -> bool {
    is_underscore(c) || c.is_lowercase()
  }

  // JsonPath

  named!(json_path<CompleteStr, Value>, 
    do_parse!(
      d: char!('$')                  >> 
      r: take_till!(is_end_of_value) >>
         (Value::Variable(Variable::JsonPath { path: format!("{}{}", d, r) }))
    )
  );

  #[test]
  fn parse_json_path() {
    let res = json_path("$.persons[0].gender".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::JsonPath { path: "$.persons[0].gender".to_owned() }))));
  }

  // Env

  named!(env<CompleteStr, Value>, 
    do_parse!(
      tag!("env")                 >>
      e: delimited!(tag!("["), take_while!(is_env_name_char), tag!("]")) >>
      (Value::Variable(Variable::Env { name: String::from(e.0) }))
    )
  );

  #[test]
  fn parse_env() {
    let res = env("env[HOST]".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Env { name: "HOST".to_owned() }))))
  }

  // Property

  named!(prop<CompleteStr, Value>, 
    do_parse!(
      tag!("prop")                 >>
      p: delimited!(tag!("["), take_while!(is_prop_name_char), tag!("]")) >>
      (Value::Variable(Variable::Property { name: String::from(p.0) }))
    )
  );

  #[test]
  fn parse_prop() {
    let res = prop("prop[reply_to]".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Property { name: "reply_to".to_owned() }))));
  }

  // Generators

  #[derive(Debug, PartialEq)]
  enum GeneratorType {
    Uuid,
    Int,
    Str,
    Real,
  }

  named!(generator<CompleteStr, Value>,
    alt!(gen_uuid | gen_real | gen_str | gen_int)
  );

  named!(gen_uuid<CompleteStr, Value>, 
    value!(Value::Variable(Variable::Generator{g_type: GeneratorType::Uuid}), delimited!(char!('<'), tag!("uuid"), char!('>')))
  );

  named!(gen_real<CompleteStr, Value>, 
    value!(Value::Variable(Variable::Generator{g_type: GeneratorType::Real}), delimited!(char!('<'), tag!("real"), char!('>')))
  );

  named!(gen_str<CompleteStr, Value>, 
    value!(Value::Variable(Variable::Generator{g_type: GeneratorType::Str}), delimited!(char!('<'), tag!("str"), char!('>')))
  );

  named!(gen_int<CompleteStr, Value>, 
    value!(Value::Variable(Variable::Generator{g_type: GeneratorType::Int}), delimited!(char!('<'), tag!("int"), char!('>')))
  );

  #[test]
  fn parse_gen_uuid() {
    let res = gen_uuid("<uuid>".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Generator{g_type: GeneratorType::Uuid}))));
  }

  #[test]
  fn parse_gen_int() {
    let res = gen_int("<int>".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Generator{g_type: GeneratorType::Int}))));
  }

  #[test]
  fn parse_gen_str() {
    let res = gen_str("<str>".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Generator{g_type: GeneratorType::Str}))));
  }

  #[test]
  fn parse_gen_real() {
    let res = gen_real("<real>".into());
    assert_eq!(res, Ok(("".into(), Value::Variable(Variable::Generator{g_type: GeneratorType::Real}))));
  }
}
