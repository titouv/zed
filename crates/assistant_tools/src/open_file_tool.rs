use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use assistant_tool::{ActionLog, Tool};
use gpui::{App, Entity, Task};
use language_model::LanguageModelRequestMessage;
use project::Project;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ui::IconName;
use util::markdown::MarkdownString;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OpenFileToolInput {
    /// The relative path of the file to open.
    ///
    /// This path should never be absolute, and the first component
    /// of the path should always be a root directory in a project.
    ///
    /// <example>
    /// If the project has the following root directories:
    ///
    /// - directory1
    /// - directory2
    ///
    /// If you wanna access `file.txt` in `directory1`, you should use the path `directory1/file.txt`.
    /// If you wanna access `file.txt` in `directory2`, you should use the path `directory2/file.txt`.
    /// </example>
    pub path: Arc<Path>,

    /// Line number to start reading from (1-based index)
    pub start_line: usize,

    /// Line number to end reading at (1-based index)
    pub end_line: usize,
}

pub struct OpenFileTool;

impl Tool for OpenFileTool {
    fn name(&self) -> String {
        "open-file".into()
    }

    fn needs_confirmation(&self) -> bool {
        false
    }

    fn description(&self) -> String {
        include_str!("./open_file_tool/description.md").into()
    }

    fn icon(&self) -> IconName {
        IconName::Eye
    }

    fn input_schema(&self) -> serde_json::Value {
        let schema = schemars::schema_for!(OpenFileToolInput);
        serde_json::to_value(&schema).unwrap()
    }

    fn ui_text(&self, input: &serde_json::Value) -> String {
        match serde_json::from_value::<OpenFileToolInput>(input.clone()) {
            Ok(input) => {
                let path = MarkdownString::inline_code(&input.path.display().to_string());
                let range_text = format!(" (lines {}-{})", input.start_line, input.end_line);
                format!("Open {}{}", path, range_text)
            }
            Err(_) => "Open file".to_string(),
        }
    }

    fn run(
        self: Arc<Self>,
        input: serde_json::Value,
        _messages: &[LanguageModelRequestMessage],
        project: Entity<Project>,
        action_log: Entity<ActionLog>,
        cx: &mut App,
    ) -> Task<Result<String>> {
        let input = match serde_json::from_value::<OpenFileToolInput>(input) {
            Ok(input) => input,
            Err(err) => return Task::ready(Err(anyhow!(err))),
        };

        let Some(project_path) = project.read(cx).find_project_path(&input.path, cx) else {
            return Task::ready(Err(anyhow!(
                "Path {} not found in project",
                &input.path.display()
            )));
        };

        cx.spawn(async move |cx| {
            let buffer = cx
                .update(|cx| {
                    project.update(cx, |project, cx| project.open_buffer(project_path, cx))
                })?
                .await?;

            action_log.update(cx, |log, cx| {
                log.buffer_opened(buffer, Some(input.start_line), Some(input.end_line), cx);
            })?;

            anyhow::Ok("Opened".to_string())
        })
    }
}
