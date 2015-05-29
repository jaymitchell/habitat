//
// Copyright:: Copyright (c) 2015 Chef Software, Inc.
// License:: Apache License, Version 2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use std::io::prelude::*;
use std::fs::File;
use std::collections::{HashMap, BTreeMap};
use mustache;
use rustc_serialize::json::Json;
use error::{BldrResult, BldrError};
use toml;
use ansi_term::Colour::Purple;
use pkg;
use std::env;
use discovery;

pub fn package(pkg: &str) -> BldrResult<()> {
    let package = try!(pkg::latest(pkg));

    println!("   {}: Creating srvc paths", pkg);
    try!(package.create_srvc_path());
    println!("   {}: Copying START", pkg);
    try!(package.copy_start());

    println!("   {}: Loading default data", pkg);
    let mut default_toml_file = try!(File::open(package.join_path("config/DEFAULT.toml")));
    let mut toml_data = String::new();
    try!(default_toml_file.read_to_string(&mut toml_data));
    let mut toml_parser = toml::Parser::new(&toml_data);
    let default_toml_value = try!(toml_parser.parse().ok_or(BldrError::TomlParser(toml_parser.errors)));

    let discovery_toml = match discovery::etcd::get_config(pkg) {
        Some(discovery_toml_value) => {
            toml_merge(default_toml_value, discovery_toml_value)
        },
        None => default_toml_value
    };

    println!("   {}: Overlaying environment configuration", pkg);
    let env_toml = try!(env_to_toml(pkg));
    let final_data = match env_toml {
        Some(env_toml_value) => toml_table_to_mustache(toml_merge(discovery_toml, env_toml_value)),
        None => toml_table_to_mustache(discovery_toml)
    };

    println!("   {}: Writing out configuration files", pkg);
    let config_files = try!(package.config_files());
    for config in config_files {
        let template = try!(mustache::compile_path(package.join_path(&format!("config/{}", config))));
        println!("   {}: Rendering {}", pkg, Purple.bold().paint(&config));
        let mut config_file = try!(File::create(package.srvc_join_path(&format!("config/{}", config))));
        template.render_data(&mut config_file, &final_data);
    }
    println!("   {}: Configured", pkg);
    Ok(())
}

fn env_to_toml(pkg: &str) -> BldrResult<Option<BTreeMap<String, toml::Value>>> {
    let toml_data = match env::var(&format!("BLDR_{}", pkg)) {
        Ok(val) => val,
        Err(e) => {
            debug!("Looking up environment variable BLDR_{} failed: {:?}", pkg, e);
            return Ok(None)
        }
    };
    let mut toml_parser = toml::Parser::new(&toml_data);
    let toml_value = try!(toml_parser.parse().ok_or(BldrError::TomlParser(toml_parser.errors)));
    Ok(Some(toml_value))
}

/// A completely shallow merge of two Toml tables. For v0 of Bldr, if you set any nested key,
/// you must set *all* the keys in that nesting, or your out of luck. Someday, this will need
/// to become a legitimate deep merge.
///
/// We use toml as the middle language because its implementation in rust lends itself to easy
/// cloning of even the deep data.
fn toml_merge(left: BTreeMap<String, toml::Value>, right: BTreeMap<String, toml::Value>) -> BTreeMap<String, toml::Value> {
    let mut final_map = BTreeMap::new();
    for (left_key, left_value) in left.iter() {
        match right.get(left_key) {
            Some(right_value) => { final_map.insert(left_key.clone(), right_value.clone()); },
            None => { final_map.insert(left_key.clone(), left_value.clone()); },
        }
    }
    final_map
}

pub fn toml_table_to_mustache(toml: BTreeMap<String, toml::Value>) -> mustache::Data {
    let mut hashmap = HashMap::new();
    for (key, value) in toml.iter() {
        hashmap.insert(format!("{}", key), toml_to_mustache(value.clone()));
    }
    mustache::Data::Map(hashmap)
}

pub fn toml_to_mustache(value: toml::Value) -> mustache::Data {
    match value {
        toml::Value::String(s) => mustache::Data::StrVal(format!("{}", s)),
        toml::Value::Integer(i) => mustache::Data::StrVal(format!("{}", i)),
        toml::Value::Float(i) => mustache::Data::StrVal(format!("{}", i)),
        toml::Value::Boolean(b) => mustache::Data::Bool(b),
        toml::Value::Datetime(s) => mustache::Data::StrVal(format!("{}", s)),
        toml::Value::Array(a) => toml_vec_to_mustache(a),
        toml::Value::Table(t) => toml_table_to_mustache(t),
    }
}

pub fn toml_vec_to_mustache(toml: Vec<toml::Value>) -> mustache::Data {
    let mut mvec = vec![];
    for x in toml.iter() {
        mvec.push(toml_to_mustache(x.clone()))
    }
    mustache::Data::VecVal(mvec)
}

pub fn json_to_mustache(value: Json) -> mustache::Data {
    match value {
        Json::I64(i) => mustache::Data::StrVal(format!("{}", i)),
        Json::U64(i) => mustache::Data::StrVal(format!("{}", i)),
        Json::F64(i) => mustache::Data::StrVal(format!("{}", i)),
        Json::String(s) => mustache::Data::StrVal(format!("{}", s)),
        Json::Boolean(b) => mustache::Data::Bool(b),
        Json::Array(a) => json_vec_to_mustache(a),
        Json::Object(o) => json_object_to_mustache(o),
        Json::Null => mustache::Data::StrVal("".to_string()),
    }
}

pub fn json_object_to_mustache(json: BTreeMap<String, Json>) -> mustache::Data {
    let mut hashmap = HashMap::new();
    for (key, value) in json.iter() {
        hashmap.insert(format!("{}", key), json_to_mustache(value.clone()));
    }
    mustache::Data::Map(hashmap)
}

pub fn json_vec_to_mustache(json: Vec<Json>) -> mustache::Data {
    let mut mvec = vec![];
    for x in json.iter() {
        mvec.push(json_to_mustache(x.clone()))
    }
    mustache::Data::VecVal(mvec)
}

#[cfg(test)]
mod tests {
    use super::{json_object_to_mustache, toml_table_to_mustache};
    use rustc_serialize::json::Json;
    use toml;
    use mustache;

    #[test]
    fn json_data_is_rendered_to_mustache() {
        let json_data = Json::from_str("
{
  \"daemonize\": \"no\",
  \"slaveof\": \"127.0.0.1 6380\",
  \"winks\": {
    \"left\": \"yes\",
    \"right\": \"no\",
    \"wiggle\": [
        \"snooze\",
        \"looze\"
    ]
  }
}").unwrap();
        let template = mustache::compile_str("hello {{daemonize}} for {{slaveof}} {{winks.right}} {{winks.left}} {{# winks.wiggle}} {{.}} {{/winks.wiggle}}");
        let mut bytes = vec![];
        let data = json_object_to_mustache(json_data.as_object().unwrap().clone());
        template.render_data(&mut bytes, &data);
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello no for 127.0.0.1 6380 no yes  snooze  looze ".to_string());
    }

    #[test]
    fn toml_data_is_rendered_to_mustache() {
        let toml = r#"
            daemonize = "no"
            slaveof = "127.0.0.1 6380"

            [winks]
            left = "yes"
            right = "no"
            wiggle = [ "snooze", "looze" ]
        "#;
        let toml_value = toml::Parser::new(toml).parse().unwrap();
        let template = mustache::compile_str("hello {{daemonize}} for {{slaveof}} {{winks.right}} {{winks.left}} {{# winks.wiggle}} {{.}} {{/winks.wiggle}}");
        let mut bytes = vec![];
        let data = toml_table_to_mustache(toml_value);
        template.render_data(&mut bytes, &data);
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello no for 127.0.0.1 6380 no yes  snooze  looze ".to_string());
    }
}