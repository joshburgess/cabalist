//! `cabalist-cli fmt` — Format the .cabal file.
//!
//! Performs round-trip formatting (parse + render) and optionally sorts
//! dependencies and modules alphabetically.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_opinions::config::find_and_load_config;

use crate::util;

pub fn run(file: &Option<PathBuf>, check: bool) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (original_source, _result) = util::load_and_parse(&cabal_path)?;

    // Load config for formatting preferences.
    let project_root = cabal_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = find_and_load_config(project_root);

    let mut current_source = original_source.clone();

    // Sort dependencies if configured.
    if config.formatting.sort_dependencies {
        current_source = cabalist_opinions::fmt::sort_list_field(&current_source,"build-depends");
    }

    // Sort modules if configured.
    if config.formatting.sort_modules {
        current_source = cabalist_opinions::fmt::sort_list_field(&current_source,"exposed-modules");
        current_source = cabalist_opinions::fmt::sort_list_field(&current_source,"other-modules");
    }

    // Re-parse and render to normalize (the round-trip should be clean).
    let final_result = cabalist_parser::parse(&current_source);
    let formatted = final_result.cst.render();

    if check {
        if formatted != original_source {
            eprintln!("{} needs formatting", cabal_path.display());
            return Ok(ExitCode::from(1));
        }
        println!("{} is correctly formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    if formatted == original_source {
        println!("{} is already formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    std::fs::write(&cabal_path, &formatted)?;
    println!("Formatted {}", cabal_path.display());
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use cabalist_opinions::fmt::sort_list_field;
    use cabalist_parser::ast::derive_ast;

    #[test]
    fn sort_trailing_comma_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
    aeson ^>=2.2,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_leading_comma_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
      text ^>=2.0
    , base ^>=4.17
    , aeson ^>=2.2
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_single_line_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends: text ^>=2.0, base ^>=4.17, aeson ^>=2.2
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_modules_no_comma() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules:
    Zebra
    Alpha
    Middle
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "exposed-modules");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.exposed_modules, vec!["Alpha", "Middle", "Zebra"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_already_sorted_is_noop() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    aeson ^>=2.2,
    base ^>=4.17,
    text ^>=2.0,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        assert_eq!(sorted, source, "already sorted should be a no-op");
    }

    #[test]
    fn sort_multiple_sections() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
  default-language: GHC2021

executable my-exe
  main-is: Main.hs
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        let lib_deps: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(lib_deps, vec!["base", "text"]);

        let exe = &ast.executables[0];
        let exe_deps: Vec<&str> = exe.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(exe_deps, vec!["base", "text"]);

        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_idempotent() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
    aeson ^>=2.2,
  default-language: GHC2021
";
        let first = sort_list_field(source, "build-depends");
        let second = sort_list_field(&first, "build-depends");
        assert_eq!(first, second, "sorting must be idempotent");
    }
}
