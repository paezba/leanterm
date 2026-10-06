# Leanterm

A lean, offline terminal.

> [!IMPORTANT]
> This repository ([paezba/leanterm](https://github.com/paezba/leanterm)) is a standalone fork of [warpdotdev/warp](https://github.com/warpdotdev/warp). It is not affiliated with Warp and does not track upstream.
> Leanterm is not affiliated with, endorsed by or sponsored by Denver Technologies, Inc. (Warp). "Warp" and the Warp logos are their trademarks and are used here only to identify the upstream project. If the trademark owner has any concern about the name or branding, please [open an issue](https://github.com/paezba/leanterm/issues) and we will rename the project.

Leanterm keeps Warp's terminal and local editing features and removes everything that needs an account, a server or an AI model. The `leanterm` binary builds `Leanterm.app`, which starts without a login screen and makes no outbound network connections on launch.

### What was removed

- **AI**: the built-in agent, agent view and agent tabs, Warp AI, MCP, bundled skills, AI settings and keybindings, and the AI-related local control surfaces.
- **Cloud and accounts**: login and auth, Warp Drive and cloud objects, sharing, teams, shared sessions, settings sync, the `ServerApi`/GraphQL layer, and the cloud/AI database tables (dropped by a migration).
- **Telemetry and services**: telemetry, crash reporting (Sentry), autoupdate, the changelog, onboarding, tips, referrals and experiments.
- **Other surfaces**: the TUI, the remote SSH extension (`remote_server` crate and daemon), `leanterm_cli` cloud subcommands, non-OSS channel binaries and upstream release tooling.
- **Dead code**: most feature flags (deleted, or folded into always-on code), unused crates and dependencies, and unreferenced AI/promo assets.

In total the fork removes about 1.2 million lines.

### What remains

The terminal (blocks, completions, tabs and panes, vertical tabs by default, shell integration for SSH and subshells), the code editor with LSP and code review, local Markdown notebooks, local YAML workflows, local control (`leantermctl`), themes and settings. macOS, Linux and Windows code is kept; the WASM/web target is not maintained.

### Building

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run Leanterm
```

See [AGENTS.md](AGENTS.md) for the engineering guide.

## Licensing

Warp's UI framework (the `leanterm_ui_core` and `leanterm_ui` crates) are licensed under the [MIT license](LICENSE-MIT).

The rest of the code in this repository is licensed under the [AGPL v3](LICENSE-AGPL).

## Open Source Dependencies

We'd like to call out a few of the open source projects Leanterm builds on:

- [Tokio](https://github.com/tokio-rs/tokio)
- [NuShell](https://github.com/nushell/nushell)
- [Fig Completion Specs](https://github.com/withfig/autocomplete)
- [Warp Server Framework](https://github.com/seanmonstar/warp)
- [Alacritty](https://github.com/alacritty/alacritty)
- [Hyper HTTP library](https://github.com/hyperium/hyper)
- [FontKit](https://github.com/servo/font-kit)
- [Core-foundation](https://github.com/servo/core-foundation-rs)
- [Smol](https://github.com/smol-rs/smol)
