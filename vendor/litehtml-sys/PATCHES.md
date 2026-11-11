# litehtml-sys patches

This is a vendored copy of `litehtml-sys` 0.2.5, patched so the upstream
litehtml C/C++ sources build on Windows with MSVC.

Changes:

- `vendor/litehtml/src/gumbo/*.c`: use `<string.h>` and map `strcasecmp` /
  `strncasecmp` to `_stricmp` / `_strnicmp` on Windows instead of including the
  Unix-only `<strings.h>`.
- `build.rs`: pass `/utf-8` and `/permissive-` to MSVC, and link `msvcprt`
  instead of `stdc++` on MSVC targets.
- `vendor/litehtml/src/media_query.cpp`: use `!` instead of the alternative
  token `not`, which MSVC does not accept in this context.
- `csrc/litehtml_c.cpp`: pass a `std::string` to `parse_css_stylesheet` so the
  existing `std::string` template instantiation is used. MSVC otherwise emits
  an unresolved reference for the `const char*` instantiation.

The vendored copy is only enabled through `[patch.crates-io]` in the root
`Cargo.toml`. Re-apply these patches when updating litehtml-sys.
