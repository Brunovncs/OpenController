//! DSU status shared by the Lab snapshot.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    pub running: bool,
    pub port: u16,
    pub clients: usize,
    pub error: Option<String>,
}
