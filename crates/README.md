# Crate layout

The workspace keeps crate roles explicit:

- `formats/` contains reusable public format libraries. These crates may be
  maintained as independent Git submodules and must not depend on Toolkit UI or
  runtime crates.
- `runtime/` contains private application support code: editor cores, renderers,
  native integration, and Web Workers. Group related crates under the format or
  feature name.
- `tools/` contains the Dioxus feature crate for each Toolkit page. Tool crates
  may depend on public format libraries, runtime crates, and `toolkit-ui`.
- `ui/` contains shared, format-agnostic UI infrastructure.

New reusable libraries belong in `formats/<crate-name>`. New application-only
workers or helper crates belong in `runtime/<feature>/`, while the corresponding
page remains in `tools/<feature>-tool`.

The intended dependency direction is:

```text
formats -> runtime -> tools -> toolkit-app
    \----------------> tools
ui -----------------> tools
```
