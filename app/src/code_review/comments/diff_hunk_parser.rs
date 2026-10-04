//! Utilities for parsing unified diff hunks and extracting specific line content.



impl std::fmt::Display for DiffHunkParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffHunkParseError::EmptyHunk => write!(f, "Empty diff hunk"),
            DiffHunkParseError::InvalidHeader(err) => write!(f, "Invalid header: {err}"),
            DiffHunkParseError::UnexpectedHunkHeader { line_index } => {
                write!(f, "Unexpected hunk header at line index {line_index}")
            }
            DiffHunkParseError::LineNotFound { target_line } => {
                write!(f, "Target line {target_line} not found in hunk")
            }
        }
    }
}

impl From<anyhow::Error> for DiffHunkParseError {
    fn from(err: anyhow::Error) -> Self {
        DiffHunkParseError::InvalidHeader(err)
    }
}

#[cfg(test)]
#[path = "diff_hunk_parser_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) enum DiffHunkParseError {
    EmptyHunk,
    InvalidHeader(anyhow::Error),
    UnexpectedHunkHeader { line_index: usize },
    LineNotFound { target_line: usize },
}
