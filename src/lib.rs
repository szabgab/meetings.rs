use serde::{Deserialize, Serialize};

pub mod db;
pub use db::*;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub email: String,
    pub name: String,
    pub code: String,
    pub verified: bool,
    pub date: String,
}

// TODO is there a better way to set the id of the event to the filename?
#[derive(Deserialize, Serialize, Debug)]
pub struct Event {
    #[serde(default = "get_empty_string")]
    pub id: String,
    pub title: String,
    pub date: String,
    pub location: String,
    pub group_id: usize,
    pub body: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Group {
    #[serde(default = "get_empty_string")]
    pub id: String,
    pub name: String,
    pub location: String,
    pub description: String,
}

fn get_empty_string() -> String {
    String::new()
}