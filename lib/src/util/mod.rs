use failure::Error;
use std::fs::File;
use std::io::Read;

pub fn read_file(path: &str) -> Result<String, Error> {
  let mut file = File::open(path)?;
  let mut contents = String::new();
  file.read_to_string(&mut contents);
  Ok(contents)
}
