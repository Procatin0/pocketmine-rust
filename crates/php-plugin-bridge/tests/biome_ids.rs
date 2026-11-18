//! PHP behaviour test for `pocketmine\data\bedrock\BiomeIds`.
//!
//! PM5 ships the full Bedrock biome catalog (90 constants). A plugin calling
//! `BiomeIds::MESA` used to fail with "undefined constant" because only the
//! handful of ids the bridge registered were declared. `POCKETMINE_PHP`
//! selects the interpreter (same variable the bridge uses); when no PHP is
//! available the test reports a skip instead of failing on machines that only
//! build Rust.

use std::path::{Path, PathBuf};
use std::process::Command;

use php_plugin_bridge::runtime::bootstrap::write_runtime;

fn php_binary() -> String {
    std::env::var("POCKETMINE_PHP").unwrap_or_else(|_| "php".to_string())
}

/// Runs `tests/fixtures/<name>` against a freshly written runtime.
fn run_fixture(name: &str) -> Option<(bool, String)> {
    let php = php_binary();
    match Command::new(&php).arg("-v").output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            eprintln!(
                "skipping PHP fixture '{name}': `{php} -v` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return None;
        }
        Err(error) => {
            eprintln!("skipping PHP fixture '{name}': no PHP interpreter ({error})");
            return None;
        }
    }

    let runtime_dir = std::env::temp_dir().join(format!(
        "pocketmine-rust-php-tests-{}-{}",
        std::process::id(),
        name.replace('.', "-")
    ));
    write_runtime(&runtime_dir).expect("write PHP runtime");

    let fixture = fixture_path(name);
    let output = Command::new(&php)
        .arg(&fixture)
        .arg(&runtime_dir)
        .output()
        .expect("run PHP fixture");

    let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
    report.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), report))
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn biome_ids_declares_the_full_catalog_and_registry_resolves_it() {
    let Some((success, report)) = run_fixture("biome_ids.php") else {
        return;
    };
    assert!(success, "PHP fixture failed:\n{report}");
    assert!(report.contains("ALL OK"), "unexpected fixture output:\n{report}");
}
