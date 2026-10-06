//! Helpers for constructing repo detection calls.

use std::future::Future;
use std::path::PathBuf;

#[cfg(not(target_family = "wasm"))]
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::repositories::RepoDetectionSource;
use leanterm_ui::AppContext;
#[cfg(not(target_family = "wasm"))]
use leanterm_ui::SingletonEntity;

/// Detects the git repository root for the given working directory.
///
/// Callers that only need the `DetectedGitRepo` event side effect may drop the returned future:
/// detection runs on a task spawned inside [`DetectedRepositories`], so it completes regardless.
#[cfg(not(target_family = "wasm"))]
pub fn detect_possible_git_repo(
    active_directory: &str,
    source: RepoDetectionSource,
    ctx: &mut AppContext,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    DetectedRepositories::handle(ctx).update(ctx, |repos, ctx| {
        repos.detect_possible_git_repo(active_directory, source, ctx)
    })
}

/// Repository detection is not available in WASM builds because
/// `DetectedRepositories` is not registered there.
#[cfg(target_family = "wasm")]
pub fn detect_possible_git_repo(
    _active_directory: &str,
    _source: RepoDetectionSource,
    _ctx: &mut AppContext,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    futures::future::ready(None)
}
