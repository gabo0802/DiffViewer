use crate::store::DiffSet;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub mod git;
pub mod p4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportTarget {
    GitWorkingTree { repo_path: String },
    GitCommit { repo_path: String, rev: String },
    GitPullRequest { repo_path: String, pr_id: String, target_branch: String, pr_title: Option<String> },
    GitStash { repo_path: String, stash_id: String },
    P4Pending { change: String, cwd: Option<String> },
    P4Shelved { change: String, cwd: Option<String> },
    P4Submitted { change: String, cwd: Option<String> },
}

pub trait ScmProvider {
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    fn import_target(&self, conn: &Connection, workspace_id: &str, target: &ImportTarget) -> Result<String, String>;
    fn replace_target(&self, conn: &Connection, diffset: &DiffSet, target: &ImportTarget) -> Result<(), String>;
}
