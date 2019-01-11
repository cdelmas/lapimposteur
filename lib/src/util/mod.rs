#[cfg(test)]
mod tests {

  use jsonpath::Selector;
  use model::imposter::{Body, Headers, Message, Route};
  use rustache::{HashBuilder, Render};
  use serde_json::Value;
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

    let json: Value = serde_json::from_str(&msg.body.0).unwrap();
    let selector = Selector::new("$.ex[0]").unwrap();

    let data: Vec<String> = selector.find(&json).map(|t| t.to_string()).collect();

    assert_eq!(data, vec!["{\"data\":\"123\"}"]);
  }

  #[test]
  fn rustache() {
    let data = HashBuilder::new().insert("$__ex[0]__data", "your name");
    let mut out = Cursor::new(Vec::new());
    data.render("{{ $__ex[0]__data }}", &mut out).unwrap();
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
  // value is either : a literal (digits -> int, or digits . digits -> double, "string", or object literal (json?) ) -> for a future evolution?
  //       or : a json path
  //       or : a property reference
  //       or : an environment variable reference
  //       or : a generator


  named!(var_map<CompleteStr, HashMap<&str, Variable> >,
    dbg_dmp!(map!(many0!(var_decl), |v:Vec<(&str,Variable)>| v.into_iter().collect()))
  );

  #[test]
  fn parse_var_file() {
    let res = var_map("id = $.id\nhost = env[HOST]\ncorrId = prop[correlation_id]\nrandomStr = <str>\n".into());
    assert_eq!(
      res, 
      Ok(("".into(), hashmap!( 
        "id" => Variable::JsonPath{path: "$.id".to_owned()},
        "host" => Variable::Env{name: "HOST".to_owned()},
        "corrId" => Variable::Property{name: "correlation_id".to_owned()},
        "randomStr" => Variable::Generator{g_type: GeneratorType::Str}
      )))
    );
  }

  named!(var_decl    <CompleteStr,(&str,Variable)>,
    do_parse!(
      opt!(space) >>
      k: var_name >>
      ws!(tag!("=")) >> 
      v: alt!(json_path | prop | env | generator) >>
      opt!(space) >>
      eol >>
      (k.0,v)
    )
  );

 #[test]
  fn parse_var_decl() {
    let res = var_decl("id = $.id\n".into());
    assert_eq!(res, Ok(("".into(), ("id", Variable::JsonPath{path: "$.id".to_owned()}))));
  } 
 
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
  named!(var_name<CompleteStr, CompleteStr>, take_while!(is_var_name_char));

  #[test]
  fn parse_var_name(){
    let res = var_name("id0  = ".into());
    
    assert_eq!(res, Ok(("  = ".into(), "id0".into())));
  }

  named!(space_or_line_ending<CompleteStr,CompleteStr>, is_a!(" \r\n"));

  fn is_underscore(c: char) -> bool {
    c == '_'
  }

  fn is_end_of_value(c: char) -> bool {
    c.is_whitespace() || c == '\n'
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

  named!(json_path<CompleteStr, Variable>, 
    do_parse!(
      d: char!('$')                  >> 
      r: take_till!(is_end_of_value) >>
         (Variable::JsonPath { path: format!("{}{}", d, r) })
    )
  );

  #[test]
  fn parse_json_path() {
    let res = json_path("$.persons[0].gender".into());
    assert_eq!(res, Ok(("".into(), Variable::JsonPath { path: "$.persons[0].gender".to_owned() })));
  }

  // Env

  named!(env<CompleteStr, Variable>, 
    do_parse!(
         tag!("env")                 >>
         e: delimited!(tag!("["), take_while!(is_env_name_char), tag!("]")) >>
         (Variable::Env { name: String::from(e.0) })
    )
  );

  #[test]
  fn parse_env() {
    let res = env("env[HOST]".into());
    assert_eq!(res, Ok(("".into(), Variable::Env { name: "HOST".to_owned() })))
  }

  // Property

  named!(prop<CompleteStr, Variable>, 
    do_parse!(
         tag!("prop")                 >>
         p: delimited!(tag!("["), take_while!(is_prop_name_char), tag!("]")) >>
         (Variable::Property { name: String::from(p.0) })
    )
  );

  #[test]
  fn parse_prop() {
    let res = prop("prop[reply_to]".into());
    assert_eq!(res, Ok(("".into(), Variable::Property { name: "reply_to".to_owned() })))
  }

  // Generators

  #[derive(Debug, PartialEq)]
  enum GeneratorType {
    Uuid,
    Int,
    Str,
    Real,
  }

  named!(generator<CompleteStr, Variable>,
    alt!(gen_uuid | gen_real | gen_str | gen_int)
  );

  named!(gen_uuid<CompleteStr, Variable>, 
    value!(Variable::Generator{g_type: GeneratorType::Uuid}, delimited!(char!('<'), tag!("uuid"), char!('>')))
  );

  named!(gen_real<CompleteStr, Variable>, 
    value!(Variable::Generator{g_type: GeneratorType::Real}, delimited!(char!('<'), tag!("real"), char!('>')))
  );

  named!(gen_str<CompleteStr, Variable>, 
    value!(Variable::Generator{g_type: GeneratorType::Str}, delimited!(char!('<'), tag!("str"), char!('>')))
  );

  named!(gen_int<CompleteStr, Variable>, 
    value!(Variable::Generator{g_type: GeneratorType::Int}, delimited!(char!('<'), tag!("int"), char!('>')))
  );

  #[test]
  fn parse_gen_uuid() {
    let res = gen_uuid("<uuid>".into());
    assert_eq!(res, Ok(("".into(), Variable::Generator{g_type: GeneratorType::Uuid})));
  }

  #[test]
  fn parse_gen_int() {
    let res = gen_int("<int>".into());
    assert_eq!(res, Ok(("".into(), Variable::Generator{g_type: GeneratorType::Int})));
  }

  #[test]
  fn parse_gen_str() {
    let res = gen_str("<str>".into());
    assert_eq!(res, Ok(("".into(), Variable::Generator{g_type: GeneratorType::Str})));
  }

  #[test]
  fn parse_gen_real() {
    let res = gen_real("<real>".into());
    assert_eq!(res, Ok(("".into(), Variable::Generator{g_type: GeneratorType::Real})));
  }
}
