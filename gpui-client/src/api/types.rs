//! Rust mirrors of the wire types defined in `src/types/diff.ts`.
//!
//! We keep field names aligned with the TypeScript shapes and lean on
//! `#[serde(default)]` so optional / newly-added fields don't break parsing
//! when the server is a few versions ahead.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LineType {
    Add,
    Delete,
    Normal,
    Hunk,
    Remove,
    Context,
    Header,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffLine {
    #[serde(rename = "type")]
    pub kind: LineType,
    pub content: String,
    #[serde(default, rename = "oldLineNumber")]
    pub old_line_number: Option<u32>,
    #[serde(default, rename = "newLineNumber")]
    pub new_line_number: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffChunk {
    pub header: String,
    #[serde(rename = "oldStart")]
    pub old_start: u32,
    #[serde(rename = "oldLines")]
    pub old_lines: u32,
    #[serde(rename = "newStart")]
    pub new_start: u32,
    #[serde(rename = "newLines")]
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffFile {
    pub path: String,
    #[serde(default, rename = "oldPath")]
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    #[serde(default)]
    pub chunks: Vec<DiffChunk>,
    #[serde(default, rename = "isGenerated")]
    pub is_generated: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiffSide {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BaseMode {
    Direct,
    MergeBase,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffResponse {
    pub commit: String,
    #[serde(default)]
    pub files: Vec<DiffFile>,
    #[serde(default, rename = "ignoreWhitespace")]
    pub ignore_whitespace: Option<bool>,
    #[serde(default, rename = "isEmpty")]
    pub is_empty: Option<bool>,
    #[serde(default, rename = "openInEditorAvailable")]
    pub open_in_editor_available: Option<bool>,
    #[serde(default, rename = "baseCommitish")]
    pub base_commitish: Option<String>,
    #[serde(default, rename = "targetCommitish")]
    pub target_commitish: Option<String>,
    #[serde(default, rename = "requestedBaseCommitish")]
    pub requested_base_commitish: Option<String>,
    #[serde(default, rename = "requestedTargetCommitish")]
    pub requested_target_commitish: Option<String>,
    #[serde(default, rename = "requestedBaseMode")]
    pub requested_base_mode: Option<BaseMode>,
    #[serde(default, rename = "clearComments")]
    pub clear_comments: Option<bool>,
    #[serde(default, rename = "repositoryId")]
    pub repository_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DiffLineRange {
    Single(u32),
    Range { start: u32, end: u32 },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffCommentPosition {
    pub side: DiffSide,
    pub line: DiffLineRange,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffCommentMessage {
    pub id: String,
    pub body: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffCommentCodeSnapshot {
    pub content: String,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiffCommentThread {
    pub id: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub position: DiffCommentPosition,
    #[serde(default, rename = "codeSnapshot")]
    pub code_snapshot: Option<DiffCommentCodeSnapshot>,
    pub messages: Vec<DiffCommentMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneratedStatusResponse {
    #[serde(default)]
    pub path: String,
    #[serde(default, rename = "ref")]
    pub git_ref: String,
    #[serde(default, rename = "isGenerated")]
    pub is_generated: bool,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommentsJsonResponse {
    pub version: u64,
    #[serde(default)]
    pub threads: Vec<DiffCommentThread>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevisionOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitInfo {
    pub hash: String,
    #[serde(rename = "shortHash")]
    pub short_hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevisionsResponse {
    #[serde(default, rename = "specialOptions")]
    pub special_options: Vec<RevisionOption>,
    #[serde(default)]
    pub branches: Vec<BranchInfo>,
    #[serde(default)]
    pub commits: Vec<CommitInfo>,
    #[serde(default, rename = "originDefaultBranch")]
    pub origin_default_branch: Option<String>,
    #[serde(default, rename = "resolvedBase")]
    pub resolved_base: Option<String>,
    #[serde(default, rename = "resolvedTarget")]
    pub resolved_target: Option<String>,
}
