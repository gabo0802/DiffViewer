use serde::Deserialize;

#[derive(Deserialize)]
pub struct OpenTwoWay {
  pub left_path: String,
  pub right_path: String,
  pub title: Option<String>,
}

// TODO: implement parsing from argv / OS events and routing into workspace_controller
