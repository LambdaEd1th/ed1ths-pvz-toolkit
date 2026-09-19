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

Tool-drawer actions share typography tokens in `crates/ui/toolkit-ui/assets/tokens.css`.
Text buttons, file-input labels, and generated icon captions use the same font,
size, weight, spacing, and line height. Broad control resets must use zero-
specificity `:where(...)` selectors, so they cannot override component typography.
Do not repair a reset conflict with a one-off button font override.

Hover hints share an application-owned tooltip layer. Native `title` strings are
normalized to `data-ui-tooltip`, retaining icon captions and accessible names.
Hints close on pointer exit, activation, Escape, scrolling, focus loss, window
changes, hidden/removed targets, and overlays; pending delayed hints are cancelled
too. The same lifecycle tests run on Chromium and WebKit.

The PAM editor's **Export all Sprites** button (left of **Save PAM**) exports the
document's loaded image assets as `<name>_sprites.zip`. PNGs retain their original
pixel dimensions and alpha, independently of timeline transforms, display scale,
and visibility filters. Image names are normalized and made safe, collisions get
numbered suffixes, and unavailable images are listed in `_missing-images.txt`.
Asset export does not change the PAM's saved/dirty state. Check the real download
flow against a served Web build with `UI_TEST_URL=http://127.0.0.1:8097 npm run
test:pam-images` in `scripts/ui-tests`.

The browser regression check runs in CI on Chromium and WebKit, using the real
tool styles in multiple load orders, light/dark themes, and desktop/mobile widths:

```sh
cd scripts/ui-tests
npm ci
npx playwright install chromium webkit
npm test
```

The RSB address bar also opens a **Resource Explorer**. It discovers
v3/v4 embedded resource descriptions and packaged `RESOURCES.RTON` / `.NEWTON`
manifests, or accepts an imported RTON, NEWTON, or resource-description JSON.
Logical paths form a file-manager-style directory tree, with breadcrumbs,
back/forward/up navigation, virtualized list/icon views, and an optional details
pane. Search covers the current folder and its descendants; group, type, and
mapping-state filters retain the matching folder hierarchy. Resource variants
remain distinct even when they share a path. Directory labels preserve manifest
path casing (preferring a lowercase source spelling when manifests disagree),
while matching and navigation remain case-insensitive. Physical file names are
not renamed. Logical IDs can be located in
the existing physical file browser. Atlas children resolve through their parent texture
and expose their crop rectangle; they are not presented as independent files.
Missing, ambiguous, program-generated, and unlisted resources are distinguished.
Matching uses complete paths, subgroup context, and file extensions, never
basename-only guesses. Pending file edits and removals are reflected in the
mapping; importing a manifest does not modify the archive or rewrite its IDs.
All discovered RTON, NEWTON and embedded manifests are merged together. Matching
definitions complement missing fields and retain every source; unique entries
and conflicting definitions are preserved rather than replaced by load order.
NEWTON's omitted zero atlas coordinates are interpreted as zero when crop sizes
are present. The footer lists all merged manifests.

On the Web, opening an RSB retains its browser `File` handle and reads only the
metadata and small packet headers. Resource browsing and exports read individual
RSG slices on demand, so multiple gigabyte-sized archives do not reside in WASM
memory. Unmodified **Save as** downloads the original file directly. Metadata is
limited to 128 MiB and individual reads to 256 MiB. Saving edits composes a new
browser File from rebuilt metadata, edited packets, and unchanged slices of the
original file, without loading the whole archive into WASM. Oversized individual
packets still require the desktop app; RSB offsets have a 4 GiB format limit.

The resource explorer supports **Replace content**, **Edit definition**, **Add
definition**, and **Delete definition**, followed by **Save RSB**. Definition
edits synchronize matching in-archive RTON/NEWTON and embedded descriptions,
retain unknown RTON fields and existing slots, and update child references when
an atlas ID changes. External imported manifests remain mapping-only. Path/group
edits affect logical definitions, not physical file placement. Deleting a
definition retains its underlying file and is blocked for referenced atlases.
New ordinary files can accompany definitions in existing RSG packets. PTX and
atlas-child replacement requires same-size PNG/WebP/JPEG images and re-encodes
the original texture format; lossy formats can affect the whole atlas. Crop
rectangles are bounds-checked. PAM, level, and script references are not rewritten.
Changes are applied atomically after validation and remain pending until saved.

Resource actions support checkbox selection, Ctrl/Cmd toggling, Shift ranges,
and select-all. Exporting a folder includes all descendants. Single selections
save directly; batches use a ZIP with logical paths, collision-safe variant
names, and an error report for skipped resources (512 MiB per batch). Ordinary
files preserve their original bytes; atlas children are cropped to transparent
PNGs at full resolution. Double-click an image to preview it at fit/100–400%
zoom, or double-click a supported file to open its existing Toolkit workspace.
The **Open with** menu also supports ambiguous extensions such as JSON/XML.
Opening a PAM automatically resolves image IDs, prefers the matching group,
locale, resolution and dimensions, crops its atlas children, and passes all
images to the PAM editor in memory. Equally ranked matches and missing images
are reported instead of guessed. Each batch reuses a bounded RSG/atlas cache.

Optional real-sample browser coverage (after building and serving the Web app):

```sh
RSB_RESOURCE_REAL_SAMPLE=/path/to/main.rsb UI_TEST_URL=http://127.0.0.1:8097 \
  npm --prefix scripts/ui-tests run test:resources

# Two large sparse copies (1.27 GB / 1 GB); source files remain unchanged.
RSB_RESOURCE_REAL_SAMPLE=/path/to/main.rsb RSB_SECOND_SAMPLE=/path/to/another.rsb \
  RSB_LARGE_TEST=1 UI_TEST_URL=http://127.0.0.1:8097 \
  npm --prefix scripts/ui-tests run test:rsb-tabs
```

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
