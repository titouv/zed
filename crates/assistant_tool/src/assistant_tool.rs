mod tool_registry;
mod tool_working_set;

use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use anyhow::Result;
use collections::{HashMap, HashSet};
use gpui::{App, Context, Entity, SharedString, Task};
use icons::IconName;
use language::Buffer;
use language_model::LanguageModelRequestMessage;
use project::Project;

pub use crate::tool_registry::*;
pub use crate::tool_working_set::*;

pub fn init(cx: &mut App) {
    ToolRegistry::default_global(cx);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub enum ToolSource {
    /// A native tool built-in to Zed.
    Native,
    /// A tool provided by a context server.
    ContextServer { id: SharedString },
}

/// A tool that can be used by a language model.
pub trait Tool: 'static + Send + Sync {
    /// Returns the name of the tool.
    fn name(&self) -> String;

    /// Returns the description of the tool.
    fn description(&self) -> String;

    /// Returns the icon for the tool.
    fn icon(&self) -> IconName;

    /// Returns the source of the tool.
    fn source(&self) -> ToolSource {
        ToolSource::Native
    }

    /// Returns true iff the tool needs the users's confirmation
    /// before having permission to run.
    fn needs_confirmation(&self) -> bool;

    /// Returns the JSON schema that describes the tool's input.
    fn input_schema(&self) -> serde_json::Value {
        serde_json::Value::Object(serde_json::Map::default())
    }

    /// Returns markdown to be displayed in the UI for this tool.
    fn ui_text(&self, input: &serde_json::Value) -> String;

    /// Runs the tool with the provided input.
    fn run(
        self: Arc<Self>,
        input: serde_json::Value,
        messages: &[LanguageModelRequestMessage],
        project: Entity<Project>,
        action_log: Entity<ActionLog>,
        cx: &mut App,
    ) -> Task<Result<String>>;
}

impl Debug for dyn Tool {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tool").field("name", &self.name()).finish()
    }
}

/// Tracks actions performed by tools in a thread
#[derive(Debug)]
pub struct ActionLog {
    /// Buffers that user manually added to the context, and whose content has
    /// changed since the model last saw them.
    stale_buffers_in_context: HashSet<Entity<Buffer>>,
    /// Buffers that we want to notify the model about when they change.
    tracked_buffers: HashMap<Entity<Buffer>, TrackedBuffer>,
    /// Has the model edited a file since it last checked diagnostics?
    edited_since_project_diagnostics_check: bool,
}

#[derive(Debug)]
struct TrackedBuffer {
    version: clock::Global,
    // Store multiple ranges per buffer
    ranges: Vec<(Option<usize>, Option<usize>)>, // (start_line, end_line) pairs
}

impl Default for TrackedBuffer {
    fn default() -> Self {
        Self {
            version: clock::Global::default(),
            ranges: Vec::new(),
        }
    }
}

impl ActionLog {
    /// Creates a new, empty action log.
    pub fn new() -> Self {
        Self {
            stale_buffers_in_context: HashSet::default(),
            tracked_buffers: HashMap::default(),
            edited_since_project_diagnostics_check: false,
        }
    }

    /// Tracks a buffer as open so we can include it in context
    pub fn buffer_opened(
        &mut self, 
        buffer: Entity<Buffer>, 
        start_line: Option<usize>, 
        end_line: Option<usize>, 
        cx: &mut Context<Self>
    ) {
        let tracked_buffer = self.tracked_buffers.entry(buffer.clone()).or_default();
        tracked_buffer.version = buffer.read(cx).version();
        
        // If this is a full-file request (no specific range), clear all existing ranges
        // and just track the whole file
        if start_line.is_none() && end_line.is_none() {
            if tracked_buffer.ranges.is_empty() {
                tracked_buffer.ranges.push((None, None));
            }
            return;
        }
        
        // Convert the range bounds to actual values for comparison
        let new_start = start_line.unwrap_or(1);
        let new_end = end_line.unwrap_or(usize::MAX);
        
        // Check for overlaps with existing ranges
        let mut overlapping_indices = Vec::new();
        let mut min_start = new_start;
        let mut max_end = new_end;
        
        for (i, (existing_start, existing_end)) in tracked_buffer.ranges.iter().enumerate() {
            // If this is a full file range, it encompasses everything
            if existing_start.is_none() && existing_end.is_none() {
                return; // Already tracking the entire file, no need to add more ranges
            }
            
            let existing_start = existing_start.unwrap_or(1);
            let existing_end = existing_end.unwrap_or(usize::MAX);
            
            // Check if ranges overlap or are adjacent
            // Two ranges [a,b] and [c,d] overlap if max(a,c) <= min(b,d) + 1
            // The +1 allows for adjacent ranges to be merged
            if std::cmp::max(new_start, existing_start) <= std::cmp::min(new_end, existing_end) + 1 {
                overlapping_indices.push(i);
                min_start = std::cmp::min(min_start, existing_start);
                max_end = std::cmp::max(max_end, existing_end);
            }
        }
        
        // If there are overlaps, remove the old ranges and add a merged one
        if !overlapping_indices.is_empty() {
            // Remove ranges from back to front to avoid index shifting
            for &i in overlapping_indices.iter().rev() {
                tracked_buffer.ranges.remove(i);
            }
            
            // Add the merged range
            let merged_start = if min_start == 1 { None } else { Some(min_start) };
            let merged_end = if max_end == usize::MAX { None } else { Some(max_end) };
            tracked_buffer.ranges.push((merged_start, merged_end));
        } else {
            // No overlaps, add the new range
            tracked_buffer.ranges.push((start_line, end_line));
        }
    }

    /// Mark a buffer as edited, so we can refresh it in the context
    pub fn buffer_edited(&mut self, buffers: HashSet<Entity<Buffer>>, cx: &mut Context<Self>) {
        for buffer in &buffers {
            let tracked_buffer = self.tracked_buffers.entry(buffer.clone()).or_default();
            tracked_buffer.version = buffer.read(cx).version();
        }

        self.stale_buffers_in_context.extend(buffers);
        self.edited_since_project_diagnostics_check = true;
    }

    pub fn tracked_buffers(&self) -> impl ExactSizeIterator<Item = &Entity<Buffer>> {
        self.tracked_buffers.keys()
    }
    
    /// Returns all line ranges for a tracked buffer
    pub fn tracked_buffer_ranges(&self, buffer: &Entity<Buffer>) -> Vec<(Option<usize>, Option<usize>)> {
        if let Some(tracked_buffer) = self.tracked_buffers.get(buffer) {
            tracked_buffer.ranges.clone()
        } else {
            vec![(None, None)] // Default to full file
        }
    }

    /// Notifies a diagnostics check
    pub fn checked_project_diagnostics(&mut self) {
        self.edited_since_project_diagnostics_check = false;
    }

    /// Returns true if any files have been edited since the last project diagnostics check
    pub fn has_edited_files_since_project_diagnostics_check(&self) -> bool {
        self.edited_since_project_diagnostics_check
    }

    /// Takes and returns the set of buffers pending refresh, clearing internal state.
    pub fn take_stale_buffers_in_context(&mut self) -> HashSet<Entity<Buffer>> {
        std::mem::take(&mut self.stale_buffers_in_context)
    }
}
