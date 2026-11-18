//! PHP behavior tests for pocketmine\block\inventory classes.

use std::path::Path;
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

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let output = Command::new(&php)
        .arg(&fixture)
        .arg(&runtime_dir)
        .output()
        .expect("run PHP fixture");

    let mut report = String::from_utf8_lossy(&output.stdout).into_owned();
    report.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), report))
}

#[test]
fn block_inventory_classes() {
    let Some((success, report)) = run_fixture("block_inventory.php") else {
        return;
    };
    assert!(success, "PHP fixture failed:\n{report}");
    assert!(report.contains("ALL OK"), "unexpected fixture output:\n{report}");
}
