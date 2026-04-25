The following warnings were discovered during the build. These warnings are an
indication that the packages contain code that will become an error in a
future release of Rust. These warnings typically cover changes to close
soundness problems, unintended or undocumented behavior, or critical problems
that cannot be fixed in a backwards-compatible fashion, and are not expected
to be in wide use.

Each warning should contain a link for more information on what the warning
means and how to resolve it.


To solve this problem, you can try the following approaches:


- Some affected dependencies have newer versions available.
You may want to consider updating them to a newer version to see if the issue has been fixed.

ashpd v0.8.1 has the following newer versions available: 0.9.0, 0.9.1, 0.9.2, 0.10.1, 0.10.2, 0.10.3, 0.11.0, 0.12.0, 0.12.1

- If the issue is not solved by updating the dependencies, a fix has to be
implemented by those dependencies. You can help with that by notifying the
maintainers of this problem (e.g. by creating a bug report) or by proposing a
fix to the maintainers (e.g. by creating a pull request):

  - ashpd@0.8.1
  - Repository: https://github.com/bilelmoussaoui/ashpd
  - Detailed warning command: `cargo report future-incompatibilities --id 1 --package ashpd@0.8.1`

- If waiting for an upstream fix is not an option, you can use the `[patch]`
section in `Cargo.toml` to use your own version of the dependency. For more
information, see:
https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section

The package `ashpd v0.8.1` currently triggers the following future incompatibility lints:
> warning: this function depends on never type fallback being `()`
>   --> /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ashpd-0.8.1/src/desktop/clipboard.rs:68:5
>    |
> 68 |     pub async fn set_selection(&self, session: &Session<'_>, mime_types: &[&str]) -> Result<()> {
>    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
>    |
>    = warning: this was previously accepted by the compiler but is being phased out; it will become a hard error in Rust 2024 and in a future release in all editions!
>    = note: for more information, see <https://doc.rust-lang.org/nightly/edition-guide/rust-2024/never-type-fallback.html>
>    = help: specify the types explicitly
> note: in edition 2024, the requirement `for<'de> !: Deserialize<'de>` will fail
>   --> /root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ashpd-0.8.1/src/desktop/clipboard.rs:70:16
>    |
> 70 |         self.0.call("SetSelection", &(session, options)).await?;
>    |                ^^^^
> help: use `()` annotations to avoid fallback changes
>    |
> 70 |         self.0.call::<()>("SetSelection", &(session, options)).await?;
>    |                    ++++++
> 
