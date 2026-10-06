use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(feature = "local_fs")]
use repo_metadata::repositories::DetectedRepositories;
use leanterm_ui::{Entity, ModelContext, ModelHandle, SingletonEntity, WeakModelHandle};

use super::git_repo_model::GitRepoStatusModel;
#[cfg(feature = "local_fs")]
use super::git_repo_model::new_local_git_repo_status_model;
use super::github_repo_model::GitHubRepoModel;
#[cfg(feature = "local_fs")]
use super::github_repo_model::LocalGitHubRepoModel;

// ── GitRepoModels (singleton cache) ─────────────────────────────────────────

/// Singleton model that acts as a cache / factory for per-repository
/// [`GitRepoStatusModel`] and [`GitHubRepoModel`] instances.
///
/// Multiple terminals in the same repo share a single sub-model.  When the last
/// strong handle to a sub-model is dropped, the models are torn down automatically.
pub struct GitRepoModels {
    // Per-repo status / GitHub-info models, keyed by `PathBuf` so a
    // single cache covers both local (watcher-backed) and remote (push
    // receiver) repos. Each entry stores the unified-enum handle; callers in
    // the same repo share it, and it is torn down when the last strong handle
    // is dropped.
    git_status_models: HashMap<PathBuf, WeakModelHandle<GitRepoStatusModel>>,
    github_repo_models: HashMap<PathBuf, WeakModelHandle<GitHubRepoModel>>,
}
impl GitRepoModels {
    pub fn new() -> Self {
        Self {
            git_status_models: HashMap::new(),
            github_repo_models: HashMap::new(),
        }
    }

    /// Get or create the per-repo status model for `repo`, returning a unified
    /// [`GitRepoStatusModel`] handle that dispatches to a local watcher-backed
    /// model or a remote push receiver based on the location.
    ///
    /// Multiple callers in the same repo share one model (cached by
    /// `PathBuf`); it is torn down when the last strong handle is
    /// dropped.
    ///
    /// Callers hold the returned `ModelHandle` for as long as they need updates.
    pub fn subscribe(
        &mut self,
        repo: &PathBuf,
        ctx: &mut ModelContext<Self>,
    ) -> anyhow::Result<ModelHandle<GitRepoStatusModel>> {
        if let Some(handle) = self
            .git_status_models
            .get(repo)
            .and_then(|weak| weak.upgrade(ctx))
        {
            return Ok(handle);
        }

        let repo_path = repo;
        let handle = {
            #[cfg(feature = "local_fs")]
            {
                let Some(repository_model) = DetectedRepositories::as_ref(ctx)
                    .get_local_watched_repo_for_path(repo_path, ctx)
                else {
                    anyhow::bail!(
                        "No watched repository found for path: {}",
                        repo_path.display()
                    );
                };
                new_local_git_repo_status_model(repo_path.clone(), repository_model, ctx)
            }
            #[cfg(not(feature = "local_fs"))]
            {
                anyhow::bail!(
                    "No watched repository found for path: {}",
                    repo_path.display()
                );
            }
        };

        self.git_status_models
            .insert(repo.clone(), handle.downgrade());
        Ok(handle)
    }

    /// Get or create the per-repo GitHub-info model for `repo`, returning a
    /// unified [`GitHubRepoModel`] handle that dispatches to a local
    /// `gh`-driven model or a remote push receiver based on the location.
    ///
    /// The local backend subscribes to the sibling git status model to track
    /// the current branch and fetches PR / repository info on creation, on
    /// branch change, and on a periodic timer. Multiple callers in the same
    /// repo share one model (cached by `PathBuf`).
    ///
    /// Callers hold the returned `ModelHandle` for as long as they need updates.
    pub fn subscribe_github_repo(
        &mut self,
        repo: &PathBuf,
        ctx: &mut ModelContext<Self>,
    ) -> anyhow::Result<ModelHandle<GitHubRepoModel>> {
        if let Some(handle) = self
            .github_repo_models
            .get(repo)
            .and_then(|weak| weak.upgrade(ctx))
        {
            return Ok(handle);
        }

        let repo_path = repo;
        let handle = {
            #[cfg(feature = "local_fs")]
            {
                // LocalGitHubRepoModel needs a sibling GitRepoStatusModel for
                // branch info.
                let git_status = self.subscribe(repo, ctx)?;
                let repo_path = repo_path.clone();
                let inner =
                    ctx.add_model(|ctx| LocalGitHubRepoModel::new(repo_path, git_status, ctx));
                ctx.add_model(|ctx| {
                    ctx.subscribe_to_model(&inner, |me, _, event, ctx| {
                        GitHubRepoModel::forward_event(me, event, ctx)
                    });
                    GitHubRepoModel::Local(inner)
                })
            }
            #[cfg(not(feature = "local_fs"))]
            {
                anyhow::bail!(
                    "Local GitHub repo info is unavailable without local_fs: {}",
                    repo_path.display()
                );
            }
        };

        self.github_repo_models
            .insert(repo.clone(), handle.downgrade());
        Ok(handle)
    }
}

impl Default for GitRepoModels {
    fn default() -> Self {
        Self::new()
    }
}
impl Entity for GitRepoModels {
    type Event = ();
}

impl SingletonEntity for GitRepoModels {}
