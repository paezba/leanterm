use anyhow::Result;
use diesel::sqlite::SqliteConnection;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use warp_core::command::ExitCode;

use crate::terminal::ShellHost;
