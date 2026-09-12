# Ed1th's PvZ Toolkit

A single cross-platform app for inspecting and editing Plants vs. Zombies
resources, backed by reusable, format-focused Rust libraries.

The app combines archive workspaces, the RTON editor, PAM editor/exporter,
Compiled Text editor, WEM audio converter/player, experimental BNK SoundBank
browser, and NEWTON manifest editor behind one MoeSekai-inspired home page.
More format tools can be added without turning the public libraries into an
aggregate SDK.

PAM Editor edits the native PAM document: create an animation, select a sprite
and frame, insert hold frames, edit labels and stop markers, and add/remove
instances or adjust their affine transforms and RGBA values. Editing an inherited
instance creates a keyframe on the selected frame. Frame-event JSON exposes
commands, clipping and the remaining PAM event fields. Changes support undo/redo
and unsaved-document protection. Save PAM writes the edited document; an unchanged
binary input retains its original bytes. Numeric edits follow PAM's fixed-point
precision. JSON/YAML/TOML and PNG/APNG/WebP exports remain available; PAM's former
FLA/XFL import/export is removed. This is a frame-based editor; curve-based tween
authoring and drawing new artwork are not implemented.

## Project layout

```text
apps/toolkit/             The only desktop/Web application
  assets/i18n/            External Fluent localization resources
  src/shell/              Persistent top bar, sidebar, settings, and content host
  src/pages/              App-level pages such as Home, Libraries, and About
crates/formats/
  bnk-archive/            Public Wwise BNK reader/writer and embedded media API
  compiled-text/          Public Compiled Text reader/writer
  crypt-data/             Public PopCap CryptData codec
  dzip-archive/           Public DZip archive and compression codecs
  newton-manifest/        Public NEWTON resource-manifest reader/writer
  pak-archive/            Public PopCap PAK reader/writer and zlib support
  pam-codec/              Public PAM reader/writer
  particle-codec/         Public particle-effect reader/writer
  reanim-codec/           Public Reanim reader/writer
  rsb-archive/            Public RSB/RSG archive, zlib, and PTX codecs
  rsb-patch/              Public RSB patch reader/writer and applier
  serde-rton/             Public Serde RTON reader/writer
  smf-container/          Public SMF container reader/writer
  wem-audio/              Public Wwise WEM reader, writer, and transcoders
crates/runtime/           Private application runtime crates
  compiled-text/worker/   Compiled Text Web worker
  pam/                    PAM core, formats, renderer, native window, and worker
  rsb/worker/             RSB/PTX preview worker
  rsb-patch/worker/       RSB patch worker
  rton/                   RTON editor core and worker
  smf/worker/             SMF processing worker
  wem/worker/             WEM conversion worker
crates/ui/
  toolkit-ui/             Internal design system and appearance runtime
crates/tools/
  bnk-tool/               Experimental BNK/HIRC/media editor and rebuilder
  compiled-text-tool/     Compiled Text editor, encoder, and decoder
  crypt-data-tool/        CryptData encoder and decoder
  dzip-tool/              DZip archive workspace
  newton-tool/            NEWTON manifest editor feature
  pak-tool/               PAK archive workspace
  pam-tool/               PAM application feature
  particle-tool/          Particle editor feature
  reanim-tool/            Reanim editor feature
  rsb-patch-tool/         RSB patch workspace
  rsb-tool/               RSB archive editor, extractor, and PTX preview feature
  rton-tool/              RTON application feature
  smf-tool/               SMF container workspace
  wem-tool/               WEM conversion and audio playback feature
```

The dependency direction is intentionally one-way:

```text
public format crates -> private runtime crates -> tool crates ─┐
public format crates --------------------------> tool crates ──┼-> toolkit-app
toolkit-ui ------------------------------------> tool crates ──┘
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
bnk-archive = { git = "https://github.com/LambdaEd1th/bnk-archive.git" }
compiled-text = { git = "https://github.com/LambdaEd1th/compiled-text.git" }
crypt-data = { git = "https://github.com/LambdaEd1th/crypt-data.git" }
dzip = { git = "https://github.com/LambdaEd1th/dzip-archive.git" }
newton-manifest = { git = "https://github.com/LambdaEd1th/newton-manifest.git" }
pak-archive = { git = "https://github.com/LambdaEd1th/pak-archive.git" }
pam-codec = { git = "https://github.com/LambdaEd1th/pam-codec.git" }
particle-codec = { git = "https://github.com/LambdaEd1th/particle-codec.git" }
reanim-codec = { git = "https://github.com/LambdaEd1th/reanim-codec.git" }
rsb-archive = { git = "https://github.com/LambdaEd1th/rsb-archive.git" }
rsb-patch = { git = "https://github.com/LambdaEd1th/rsb-patch.git" }
serde-rton = { git = "https://github.com/LambdaEd1th/serde-rton.git" }
smf-container = { git = "https://github.com/LambdaEd1th/smf-container.git" }
wem-audio = { git = "https://github.com/LambdaEd1th/wem-audio.git" }
```

Enable `serde-rton`'s optional `crypto` feature only when encrypted PvZ2 RTON
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

## Localization

The Toolkit shell uses external Fluent `.ftl` resources under
`apps/toolkit/assets/i18n/`. English, Simplified Chinese, French, Russian, and
Spanish are included. The app detects the system language on first launch,
falls back to English for missing messages, and persists the language selected
in Settings.

Desktop builds also discover additional locale files from a colocated
`assets/i18n/` directory. Web builds load the generated locale manifest and can
discover additional `.ftl` files when the server exposes an asset directory
listing. Each locale provides its native menu label through `language-self`.

## License

The application, internal tool crates, and reusable format crates are licensed
under AGPL-3.0-or-later. See [LICENSE](LICENSE).
