use serde_json::{self, Value};
use std::path::Path;
use std::fs;

use clap::Parser;
use glob::glob;
use regex::{Regex, Captures};

#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
  #[arg(short, long)]
  path: String,

  #[arg(short, long)]
  search_path: String,

  #[arg(short, long)]
  ignore: String,
  
  #[arg(long)]
  dry_run: bool
}

fn main() {
  let args = Args::parse();

  apply_translation(load_translations(&args.path), &args.ignore, &args.search_path, args.dry_run);
}

fn load_translations(translations_glob: &str) -> Vec<TranslateFinder> {
  glob(translations_glob)
    .expect(&format!("Panicked while reading glob: {}", translations_glob))
    .filter(|p| p.is_ok())
    .map(|entry| {
      let entry = entry.unwrap();
      TranslateFinder::new(
        get_json_value_from_fs_path(&Path::new(&entry))
        .expect(&format!("Error parsing json at {:?}", &entry))
      )
    })
    .collect()
}

fn apply_translation(translations: Vec<TranslateFinder>, ignore: &String, search_path: &String, dry_run: bool) {
  let ignore_regex = Regex::new(ignore).unwrap();

  for p in glob(search_path).expect("Failed to read glob") {
    match p {
      Ok(entry) => {
        if !ignore_regex.is_match(&entry.as_os_str().to_str().unwrap_or("")) {
          let file_as_string = fs::read_to_string(&entry).unwrap();
          for translate_finder in translations.iter() {
            if translate_finder.is_match(&file_as_string) {
              if let Some(replacement) = translate_finder.replace_with_string(&file_as_string) {
                if !dry_run {
                  let _  = fs::write(&entry, replacement);
                }
              } else {
                println!("No translations found for file {:?}", &entry);
              }
            } else {
              break;
            }
          }
        }
      }
      _ => {}
    };
  }
}

struct TranslateFinder {
  regex: Regex,
  map: serde_json::Value
}

impl TranslateFinder {
  pub fn new(map: serde_json::Value) -> Self {
    TranslateFinder { 
      regex: Regex::new(&r#"['"](?P<key>[^|]+)['"] ?\| ?translate"#).unwrap(),
      map
    }
  }

  pub fn is_match(&self, sample: &str) -> bool {
    self.regex.is_match(sample)
  }

  pub fn replace_with_string(&self, sample: &str) -> Option<String> {
    let mut did_replace = false;
    let ret_string = String::from(self.regex.replace_all(sample, |caps: &Captures| {
      format!("{}", 
        read_json_path(&self.map, &caps[1]).map(|f: Value| { 
          let s = f.as_str();
          if s.is_some() {
            did_replace = true;
          }
          return String::from(s.unwrap_or(&caps[0]));
        }).unwrap_or(String::from(""))
      )
    }));

    if did_replace {
      return Some(ret_string);
    } else { 
      return None;
    }
  }


}

fn get_json_value_from_fs_path(path: &Path) -> Option<serde_json::Value> {
  if !path.exists() {
    return None;
  }

  let json = serde_json::from_str(&fs::read_to_string(path).unwrap());

  json.ok()
}




fn read_json_path(value: &Value, path: &str) -> Option<Value> {
  path.split(".")
  .enumerate()
  .fold(None, |acc: Option<Value>, f| {
    if f.0 == 0 {
      if let Ok(i) = f.1.to_string().parse::<usize>() {
        return Some(value[i].clone());
      }
      return Some(value[f.1].clone());
    } else if acc.is_none() {
      return None;
    } else {
      if let Ok(i) = f.1.to_string().parse::<usize>() {
        return Some(acc.unwrap()[i].clone());
      }
      return Some(acc.unwrap()[f.1].clone());
    }
  })
}

#[cfg(test)]
mod tests {

use serde_json::{json};

use super::TranslateFinder;

  #[test]
  fn translate_finder_finds_translates() {

    let translate_finder = TranslateFinder::new(
      json!({
        "my_custom_string": "hello, world"
      })
    );

    assert!(translate_finder.is_match("                   {{ 'my_custom_string' | translate}}"));
    assert!(translate_finder.is_match("{{'my_custom_string'|translate}}"));
    assert!(translate_finder.is_match("{{ 'my_custom_string' | translate}}"));
  }
  
  #[test]
  fn translate_finder_finds_right_groups() {

    let translate_finder = TranslateFinder::new(
      json!({
        "my_custom_string": "hello, world"
      }));
    let replacement = translate_finder.replace_with_string("                   {{ 'my_custom_string' | translate}}");
    assert!(replacement.is_some());
    assert_eq!(replacement.unwrap(), String::from("                   hello, world"));
  }

  #[test]
  fn can_read_value_by_json_path() {

    let json_payload = json!({

      "snagHeader": "Test",
      "array": [
        {
          "name": "Embedded 0",
        },
        {
          "name": "Embedded 1",
        }
      ]

    });
    let json_path = "array.0.name";

    let u = super::read_json_path(&json_payload, json_path);
    assert_eq!(u.unwrap().as_str().unwrap(), "Embedded 0");
  }
}