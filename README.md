# Ed1th's PvZ Toolkit

A single cross-platform app for inspecting and editing Plants vs. Zombies
resources, backed by reusable, format-focused Rust libraries.

The first release combines the PAM viewer/exporter and RTON editor behind one
MoeSekai-inspired home page. More format tools can be added without turning the
public libraries into an aggregate SDK.

## Project layout

```text
apps/toolkit/             The only desktop/Web application
  src/shell/              Persistent top bar, sidebar, settings, and content host
  src/pages/              App-level pages such as Home
crates/formats/
  pam-codec/              Public PAM reader/writer
  serde_rton/             Public Serde RTON reader/writer
crates/ui/
  toolkit-ui/             Internal design system and appearance runtime
crates/tools/
  pam-tool/               PAM application feature
  rton-tool/              RTON application feature
crates/pam/               Internal PAM workflow/renderer crates
crates/rton/              Internal RTON workflow/worker crates
```

The dependency direction is intentionally one-way:

```text
pam-codec  -> PAM core/workers  -> pam-tool  ┐
serde_rton -> RTON core/worker  -> rton-tool ├-> toolkit-app
toolkit-ui ---------------------> tools -----┘
```

`toolkit-ui` is private and contains no format APIs. The app registers tools at
compile time, keeps their workspaces mounted while switching pages, and owns the
single appearance preference. Tool-specific state never flows back into the app
shell.

Tool pages use the same composition: a page header for primary actions, one
central workspace card for the format-specific editor, and temporary context
sheets for resources or inspection. PAM and RTON keep their action components
under `page_actions/`; neither tool creates a second navigation shell, permanent
sidebars, resizable layout frame, or bottom status bar.

Public libraries are intentionally independent:

```toml
[dependencies]
pam-codec = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "pam-codec" }
serde_rton = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "serde_rton" }
```

Enable `serde_rton`'s optional `crypto` feature only when encrypted PvZ2 RTON
payloads are required.

## Run

```bash
# Desktop
cargo run -p toolkit-app

# Web
dx serve --platform web -p toolkit-app
```

The Web build uses prebuilt worker packages committed with the source. Refresh
them after changing a renderer or worker crate:

```bash
./scripts/build-web-runtime.sh
```

## License

The application, internal tool crates, and reusable format crates are licensed
under AGPL-3.0-or-later. See [LICENSE](LICENSE).
