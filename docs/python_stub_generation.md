# Python Stub Generation

`python/visual_novel_engine.pyi` is currently kept as the checked-in typing contract for
the PyO3 module. CI must treat it as executable API metadata, not as informal docs:
`tests/python/test_vnengine_bindings.py::test_pyi_top_level_public_api_matches_native_module`
imports the compiled module and compares public module exports plus public class methods
against the stub. Any Rust/PyO3 public method added without a matching stub now fails the
Python test job.

## pyo3-stub-gen Evaluation

`pyo3-stub-gen` is the preferred long-term direction for generating this file from Rust
metadata because it supports PyO3 classes, functions, exceptions, and module generation
through Rust-side type information and proc macros.

It is not enabled in this crate yet because adopting it safely is an annotation sweep, not
a drop-in replacement for the existing manual stub:

- The current PyO3 upstream introspection path is still marked active development and
  requires `experimental-inspect`.
- PyO3's introspection currently has important module-shape limitations, including
  function-style `#[pymodule]` initialization.
- `pyo3-stub-gen` needs Rust-side type metadata/macros for each exported class/function to
  produce useful Python signatures, which should be introduced in a dedicated migration so
  generated stubs do not regress editor typing.

Until that migration is complete, the parity test is the CI guardrail. It keeps the stub
manual but not silent: drift fails the same pytest suite used locally and in GitHub Actions.

Sources reviewed:

- PyO3 user guide, type stub generation and introspection: https://pyo3.rs/v0.28.3/type-stub
- `pyo3-stub-gen` docs.rs crate docs: https://docs.rs/pyo3-stub-gen/latest/pyo3_stub_gen/
