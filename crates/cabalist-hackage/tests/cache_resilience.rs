use cabalist_hackage::{HackageError, HackageIndex, PackageInfo, Version};

fn sample_packages() -> Vec<PackageInfo> {
    vec![
        PackageInfo {
            name: "aeson".to_string(),
            synopsis: "Fast JSON parsing and encoding".to_string(),
            versions: vec![
                Version::parse("2.1.0.0").unwrap(),
                Version::parse("2.2.3.0").unwrap(),
            ],
            deprecated: false,
        },
        PackageInfo {
            name: "text".to_string(),
            synopsis: "Efficient packed Unicode text".to_string(),
            versions: vec![Version::parse("2.1").unwrap()],
            deprecated: false,
        },
    ]
}

#[test]
fn load_corrupted_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    std::fs::write(&path, "this is not valid json {{{").unwrap();

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), HackageError::Json(_)));
}

#[test]
fn load_truncated_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    // Write valid JSON, then truncate it.
    let idx = HackageIndex::from_packages(sample_packages());
    idx.save_to_cache(&path).unwrap();

    let full = std::fs::read_to_string(&path).unwrap();
    let truncated = &full[..full.len() / 2];
    std::fs::write(&path, truncated).unwrap();

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), HackageError::Json(_)));
}

#[test]
fn load_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    std::fs::write(&path, "").unwrap();

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), HackageError::Json(_)));
}

#[test]
fn load_valid_json_wrong_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    std::fs::write(&path, r#"{"key": "value"}"#).unwrap();

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), HackageError::Json(_)));
}

#[test]
fn load_nonexistent_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("does_not_exist.json");

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HackageError::IndexNotFound(_)
    ));
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deeply").join("nested").join("index.json");

    let idx = HackageIndex::from_packages(sample_packages());
    idx.save_to_cache(&path).unwrap();

    let loaded = HackageIndex::load_from_cache(&path).unwrap();
    assert_eq!(loaded.len(), 2);
}

#[test]
fn save_overwrites_corrupted_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    // Start with corrupted data.
    std::fs::write(&path, "corrupted garbage data!!!").unwrap();
    assert!(HackageIndex::load_from_cache(&path).is_err());

    // Overwrite with valid data.
    let idx = HackageIndex::from_packages(sample_packages());
    idx.save_to_cache(&path).unwrap();

    // Should load cleanly now.
    let loaded = HackageIndex::load_from_cache(&path).unwrap();
    assert_eq!(loaded.len(), 2);
    assert!(loaded.package_info("aeson").is_some());
}

#[test]
fn load_empty_package_list() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    std::fs::write(&path, "[]").unwrap();

    let idx = HackageIndex::load_from_cache(&path).unwrap();
    assert!(idx.is_empty());
    assert_eq!(idx.len(), 0);
}

#[test]
fn load_binary_garbage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    std::fs::write(&path, &[0xFF, 0xFE, 0x00, 0x01, 0x80, 0x90]).unwrap();

    let result = HackageIndex::load_from_cache(&path);
    assert!(result.is_err());
}

#[test]
fn round_trip_preserves_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");

    let packages = vec![PackageInfo {
        name: "special-pkg".to_string(),
        synopsis: "A package with special chars: <>&\"'".to_string(),
        versions: vec![
            Version::parse("0.0.0.1").unwrap(),
            Version::parse("1.0.0.0").unwrap(),
            Version::parse("99.99.99.99").unwrap(),
        ],
        deprecated: true,
    }];

    let idx = HackageIndex::from_packages(packages);
    idx.save_to_cache(&path).unwrap();
    let loaded = HackageIndex::load_from_cache(&path).unwrap();

    let pkg = loaded.package_info("special-pkg").unwrap();
    assert_eq!(pkg.synopsis, "A package with special chars: <>&\"'");
    assert_eq!(pkg.versions.len(), 3);
    assert!(pkg.deprecated);
}
