extern crate reqwest;
extern crate chrono;

use chrono::{DateTime, Utc};

use std::env;
use std::process::Command;
use std::str;

struct SqlConfig {
  pub host: String,
  pub port: String,
  pub database: String,
  pub user: String,
  pub password: String,
}

struct SqlDump {
  pub timestamp: DateTime<Utc>,
  pub sql: Vec<u8>, 
}

impl SqlDump {
  pub fn to_str(&self) -> &str {
    match str::from_utf8(&self.sql) {
      Ok(sql) => sql,
      Err(e) => panic!("Could not convert SqlDump to string: {}", e)
    }
  }
}

struct Dumper {
  config: SqlConfig,
}

impl Dumper {
  pub fn new(config: SqlConfig) -> Dumper {
    Dumper {
      config,
    }
  }

  pub fn exec(&self) -> SqlDump {
    let cmd = Command::new("/usr/bin/mysqldump")
      .args(&self.build_args())
      .output()
      .expect("Failed to execute mysqldump command");

    if !cmd.status.success() {
      match str::from_utf8(&cmd.stderr) {
        Ok(err) => println!("Error executing mysqldump: {}", err),
        Err(e) => println!("{}", e),
      }
    }

    SqlDump {
      timestamp: Utc::now(),
      sql: cmd.stdout,
    }
  }

  fn build_args(&self) -> [String; 5] {
    let host = format!("-h{}", self.config.host);
    let port = format!("-P{}", self.config.port);
    let user = format!("-u{}", self.config.user);
    let pass = format!("-p{}", self.config.password);
    let db = self.config.database.clone();
    [host, port, user, pass, db]
  }
}

struct GcsClient {
  pub bucket: String,
  pub oauth_token: String,
  pub dump: SqlDump,
}

impl GcsClient {
  pub fn new(bucket: String, oauth_token: String, dump: SqlDump) -> GcsClient {
    GcsClient {
      bucket,
      oauth_token,
      dump,
    }
  }

  pub fn send(&self) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::new();
    client.post(&self.build_storage_uri())
      .header("Authorization", format!("Bearer {}", self.oauth_token))
      .body(String::from(self.dump.to_str()))
      .send()
  }

  fn build_storage_uri(&self) -> String {
    let path = format!("/upload/storage/v1/b/{}/o", self.bucket);
    let query = format!("?uploadType=media&name={}.sql", self.dump.timestamp.format("%s").to_string());
    format!("https://www.googleapis.com/{}{}", path, query)
  }
}

fn main() {
  let dumper = Dumper::new(SqlConfig {
    host: env::var("MYSQL_HOST").unwrap(),
    port: env::var("MYSQL_PORT").unwrap(),
    database: env::var("MYSQL_DATABASE").unwrap(),
    user: env::var("MYSQL_USER").unwrap(),
    password: env::var("MYSQL_PASSWORD").unwrap(),
  });

  let dump = dumper.exec();

  let client = GcsClient::new(
    env::var("GCS_BUCKET").unwrap(),
    env::var("GCS_OAUTH_TOKEN").unwrap(),
    dump,
  );

  match client.send() {
    Ok(_) => println!("Finished"),
    Err(e) => panic!("Could not send sql dump to storage: {}", e),
  }
}
