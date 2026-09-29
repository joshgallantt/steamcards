//! The dependency rule, checked. Each crate's manifest is what makes an import
//! resolve, so a manifest listing the wrong crate is where the architecture
//! would break first. This reads every manifest in the workspace and fails on
//! any arrow that points the wrong way.

use std::{collections::HashMap, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Domain,
    Data,
    Di,
    Library,
    Presentation,
    App,
    /// Project tooling (`xtask`): part of the workspace, not of steamcards.
    Tooling,
}

fn layer(manifest_path: &str) -> Layer {
    let p = manifest_path.replace('\\', "/");
    if p.contains("/component/") {
        if p.ends_with("/data/Cargo.toml") {
            Layer::Data
        } else if p.ends_with("/di/Cargo.toml") {
            Layer::Di
        } else {
            Layer::Domain
        }
    } else if p.contains("/library/") {
        Layer::Library
    } else if p.contains("/ui/") {
        Layer::Presentation
    } else if p.contains("/xtask/") {
        Layer::Tooling
    } else {
        Layer::App
    }
}

/// Which layers each layer may list as a (non-dev) dependency.
fn allowed(from: Layer) -> &'static [Layer] {
    use Layer::{App, Data, Di, Domain, Library, Presentation, Tooling};
    match from {
        Domain => &[Domain],
        Data => &[Domain, Library],
        Di => &[Domain, Data, Library],
        Library => &[Library],
        Presentation => &[Domain],
        App => &[Domain, Di, Library, Presentation],
        // Tooling runs cargo and git; it uses none of steamcards.
        Tooling => &[],
    }
}

/// Read at run time, not compile time, so a moved checkout still finds itself.
#[expect(
    clippy::disallowed_methods,
    reason = "cargo tells a test where it is through the environment"
)]
fn metadata() -> std::process::Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(dir)
        .output()
        .expect("cargo metadata")
}

#[test]
fn every_dependency_points_inward() {
    let out = metadata();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let packages = meta["packages"].as_array().unwrap();

    let layers: HashMap<&str, Layer> = packages
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap(),
                layer(p["manifest_path"].as_str().unwrap()),
            )
        })
        .collect();
    assert_eq!(
        layers.len(),
        16,
        "a crate was added or removed; place it in a layer above"
    );

    let mut wrong = Vec::new();
    for p in packages {
        let name = p["name"].as_str().unwrap();
        let from = layers[name];
        for dep in p["dependencies"].as_array().unwrap() {
            let dep_name = dep["name"].as_str().unwrap();
            let is_dev = dep["kind"].as_str() == Some("dev");
            let Some(&to) = layers.get(dep_name) else {
                continue;
            };
            if dep_name == name || is_dev {
                continue;
            }
            if !allowed(from).contains(&to) {
                wrong.push(format!("{name} ({from:?}) → {dep_name} ({to:?})"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "dependencies pointing the wrong way:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn only_test_code_reaches_a_component_through_its_test_support() {
    // Doubles are for tests. A production dependency enabling `test-support`
    // would ship fakes in the binary.
    let out = metadata();
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut wrong = Vec::new();
    for p in meta["packages"].as_array().unwrap() {
        for dep in p["dependencies"].as_array().unwrap() {
            let features = dep["features"].as_array().unwrap();
            let is_dev = dep["kind"].as_str() == Some("dev");
            if !is_dev && features.iter().any(|f| f == "test-support") {
                wrong.push(format!("{} → {}", p["name"], dep["name"]));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "test-support in production:\n{}",
        wrong.join("\n")
    );
}
