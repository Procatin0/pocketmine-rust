//! PHP behaviour test for `pocketmine\world\Explosion`.
//!
//! The fixture stubs the two host calls (`world.explosion_blocks`,
//! `world.explode`) and checks the PM5 flow: validation, preview list,
//! plugin edits between `explodeA()`/`explodeB()`, empty blast without a
//! preview, and `explode()` chaining. `POCKETMINE_PHP` selects the
//! interpreter; without one the test reports a skip.

use std::path::{Path, PathBuf};
use std::process::Command;

use php_plugin_bridge::runtime::bootstrap::write_runtime;

fn php_binary() -> String {
    std::env::var("POCKETMINE_PHP").unwrap_or_else(|_| "php".to_string())
}

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
fn explosion_previews_and_queues_blasts() {
    let Some((success, report)) = run_fixture("world_explosion.php") else {
        return;
    };
    assert!(success, "PHP fixture failed:\n{report}");
    assert!(report.contains("ALL OK"), "unexpected fixture output:\n{report}");
}
