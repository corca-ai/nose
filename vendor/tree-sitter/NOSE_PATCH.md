# Tree-sitter progress error reporting

This is the crates.io `tree-sitter` 0.26.11 source (upstream commit
`64402de2857cc197ecc4ca3bc144ea91fda7e72e`, archive SHA-256
`af1c71c1c4cc0920b20d6b0f6572e7682cd07a6a2faec71067a31fa394c586df`).
The upstream MIT license is retained. All imported files are byte-identical to
that archive except `src/parser.c`; non-build `.cargo_vcs_info.json` is omitted.

The C delta updates the existing progress signal after stack condensation when
every surviving parse has positive error cost and there is no error-free finished
tree. It uses the same lower bound as the parser's existing finished-tree choice.
This includes missing-token errors, which previously left the signal false.
Grammar, recovery, stack pruning and selected trees do not change. nose uses the
signal only to stop a clean-C admission check whose result is already false.
Ordinary parsing still finishes normally. See corca-ai/nose#988 for the release
blocker and `docs/architecture.md` for the engine contract.

Changes here must retain upstream provenance, keep the delta narrow, and run the
frontend cancellation/clean-tree tests, full output equivalence, soundness and
native-package gates. The Type-4 receipt binds the vendored tree and Cargo inputs
in addition to the application crates. Vendor changes select full CI and the
Soundness Lab. Compare the archive and changed-file diff when updating this copy;
do not treat upstream source as application code for formatting or lint ratchets.
