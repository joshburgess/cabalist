//! GHC2021 default extensions list.
//!
//! Extensions that are enabled by default when using the `GHC2021` language pragma,
//! introduced in GHC 9.2.

/// Extensions enabled by default with the GHC2021 language pragma.
///
/// This list corresponds to the set of extensions that GHC 9.2+ enables
/// when `default-language: GHC2021` is specified in a `.cabal` file.
pub const GHC2021_EXTENSIONS: &[&str] = &[
    "BangPatterns",
    "BinaryLiterals",
    "ConstrainedClassMethods",
    "ConstraintKinds",
    "DeriveDataTypeable",
    "DeriveFoldable",
    "DeriveFunctor",
    "DeriveGeneric",
    "DeriveLift",
    "DeriveTraversable",
    "DoAndIfThenElse",
    "EmptyCase",
    "EmptyDataDecls",
    "EmptyDataDeriving",
    "ExistentialQuantification",
    "ExplicitForAll",
    "FieldSelectors",
    "FlexibleContexts",
    "FlexibleInstances",
    "ForeignFunctionInterface",
    "GADTSyntax",
    "GeneralisedNewtypeDeriving",
    "HexFloatLiterals",
    "ImplicitPrelude",
    "ImportQualifiedPost",
    "InstanceSigs",
    "KindSignatures",
    "MonomorphismRestriction",
    "MultiParamTypeClasses",
    "NamedFieldPuns",
    "NamedWildCards",
    "NumericUnderscores",
    "PatternGuards",
    "PolyKinds",
    "PostfixOperators",
    "RankNTypes",
    "RelaxedPolyRec",
    "ScopedTypeVariables",
    "StandaloneDeriving",
    "StandaloneKindSignatures",
    "StarIsType",
    "TraditionalRecordSyntax",
    "TupleSections",
    "TypeApplications",
    "TypeOperators",
    "TypeSynonymInstances",
];

/// Check whether a given extension name is part of GHC2021.
///
/// Comparison is case-insensitive.
pub fn is_ghc2021_extension(name: &str) -> bool {
    GHC2021_EXTENSIONS
        .iter()
        .any(|ext| ext.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghc2021_has_expected_extensions() {
        assert!(is_ghc2021_extension("ScopedTypeVariables"));
        assert!(is_ghc2021_extension("DeriveFunctor"));
        assert!(is_ghc2021_extension("ImportQualifiedPost"));
        assert!(is_ghc2021_extension("StandaloneKindSignatures"));
        assert!(is_ghc2021_extension("TypeApplications"));
        assert!(is_ghc2021_extension("BangPatterns"));
        assert!(is_ghc2021_extension("NumericUnderscores"));
    }

    #[test]
    fn ghc2021_excludes_non_members() {
        assert!(!is_ghc2021_extension("TemplateHaskell"));
        assert!(!is_ghc2021_extension("OverloadedStrings"));
        assert!(!is_ghc2021_extension("DerivingVia"));
        assert!(!is_ghc2021_extension("DataKinds"));
        assert!(!is_ghc2021_extension("GADTs"));
    }

    #[test]
    fn ghc2021_case_insensitive() {
        assert!(is_ghc2021_extension("scopedtypevariables"));
        assert!(is_ghc2021_extension("BANGPATTERNS"));
    }

    #[test]
    fn ghc2021_has_reasonable_count() {
        // GHC2021 has 46 extensions
        assert_eq!(GHC2021_EXTENSIONS.len(), 46);
    }
}
