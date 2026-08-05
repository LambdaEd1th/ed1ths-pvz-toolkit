# Ed1th's PvZ Toolkit

A single cross-platform app for inspecting and editing Plants vs. Zombies
resources, backed by reusable, format-focused Rust libraries.

The app combines the editable RSB archive workspace, RTON editor, PAM
viewer/exporter, WEM audio converter/player, experimental BNK SoundBank
browser, and NEWTON manifest editor behind one MoeSekai-inspired home page.
More format tools can be added without turning the public libraries into an
aggregate SDK.

## Project layout

```text
apps/toolkit/             The only desktop/Web application
  src/shell/              Persistent top bar, sidebar, settings, and content host
  src/pages/              App-level pages such as Home
crates/formats/
  bnk-archive/            Public Wwise BNK reader/writer and embedded media API
  newton-manifest/        Public NEWTON resource-manifest reader/writer
  pak-archive/            Public PopCap PAK reader/writer and zlib support
  pam-codec/              Public PAM reader/writer
  rsb-archive/            Public RSB/RSG archive, zlib, and PTX codecs
  serde_rton/             Public Serde RTON reader/writer
  wem-audio/              Public Wwise WEM reader, writer, and transcoders
crates/ui/
  toolkit-ui/             Internal design system and appearance runtime
crates/tools/
  bnk-tool/               Experimental BNK/HIRC/media editor and rebuilder
  newton-tool/            NEWTON manifest editor feature
  pam-tool/               PAM application feature
  rsb-tool/               RSB archive editor, extractor, and PTX preview feature
  rton-tool/              RTON application feature
  wem-tool/               WEM conversion and audio playback feature
crates/pam/               Internal PAM workflow/renderer crates
crates/rton/              Internal RTON workflow/worker crates
crates/wem/               Internal Web conversion worker
```

The dependency direction is intentionally one-way:

```text
bnk-archive --------------------> bnk-tool ──┐
newton-manifest -------------> newton-tool ─┤
pak-archive ----------------------> library  ─┤
pam-codec  -> PAM core/workers  -> pam-tool ─┤
serde_rton -> RTON core/worker  -> rton-tool ├-> toolkit-app
rsb-archive --------------------> rsb-tool  ─┤
wem-audio -> WEM worker --------> wem-tool  ─┤
toolkit-ui ---------------------> tools -----┘
```

`toolkit-ui` is private and contains no format APIs. The app registers tools at
compile time, keeps their workspaces mounted while switching pages, and owns the
single appearance preference. Tool-specific state never flows back into the app
shell.

Tool pages use the same composition: a floating toolbar for primary actions and
one central workspace card for the format-specific workflow. NEWTON uses
browser-style document tabs with group, record, and inspector cards; PAM and
RTON keep their focused editor layouts. RSB uses an archive-manager layout with
an address bar, directory tree, file table, details inspector, and a compact
operation status. PTX entries can be decoded from their archive metadata and
previewed without extracting them first. None of the tools creates a second app
navigation shell.

Public libraries are intentionally independent:

```toml
[dependencies]
bnk-archive = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "bnk-archive" }
newton-manifest = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "newton-manifest" }
pak-archive = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "pak-archive" }
pam-codec = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "pam-codec" }
rsb-archive = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "rsb-archive" }
serde_rton = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "serde_rton" }
wem-audio = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit", package = "wem-audio" }
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
