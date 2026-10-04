//! Utilities to check whether a command at the cursor position is likely a package installer command.

use string_offset::ByteOffset;
use warp_completer::parsers::simple::command_at_cursor_position;
use warp_util::path::ShellFamily;

use crate::completer::SessionContext;
use crate::terminal::alias::is_expandable_alias;

#[cfg(test)]
#[path = "package_installers_tests.rs"]
mod tests;
