use std::fs::{self, File};
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{Builder, Header};

const BUNDLE_OUTPUT: &str = "target/dx/rolxapp-web/release/web/public";
const BINARY: &str = "rolxapp-webhost";

/// Builds the web app and the host for `target`, and packs both into a tarball.
pub fn run(root: &Path, target: &str) -> Result<String, String> {
    // It becomes a directory name below, of which an old copy gets deleted.
    if target.contains(['/', '\\']) || target.contains("..") {
        return Err(format!("`{target}` is not a target triple"));
    }
    // Panic locations in dependencies would otherwise give away the user name of the build machine.
    let remap_home = dirs::home_dir()
        .map(|home| format!("--remap-path-prefix={}=/home", home.display()))
        .unwrap_or_default();

    // `dx` does not clean up: a wasm file of an earlier build would be packed as well.
    let bundle = root.join(BUNDLE_OUTPUT);
    if bundle.exists() {
        fs::remove_dir_all(&bundle)
            .map_err(|e| format!("cannot clear {}: {e}", bundle.display()))?;
    }
    run_tool(
        root,
        Command::new("dx")
            .args(["bundle", "-p", "rolxapp-web", "--web", "--release"])
            .env("RUSTFLAGS", &remap_home),
    )?;
    let mut zigbuild = Command::new("cargo");
    zigbuild
        .args(["zigbuild", "-p", BINARY, "--release", "--target", target])
        .env("RUSTFLAGS", &remap_home);
    if let Some(zig) = zig_from_python() {
        zigbuild.env("CARGO_ZIGBUILD_ZIG_PATH", zig);
    }
    run_tool(root, &mut zigbuild)?;

    if !bundle.join("index.html").is_file() {
        return Err(format!("no index.html in {}", bundle.display()));
    }
    let binary = root
        .join("target")
        .join(target)
        .join("release")
        .join(BINARY);

    // The directory is the container's build context.
    let dist = root.join("dist").join("web").join(target);
    if dist.exists() {
        fs::remove_dir_all(&dist).map_err(|e| format!("cannot clear {}: {e}", dist.display()))?;
    }
    fs::create_dir_all(&dist).map_err(|e| format!("cannot create {}: {e}", dist.display()))?;
    copy_tree(&bundle, &dist.join("public"))?;
    fs::copy(&binary, dist.join(BINARY))
        .map_err(|e| format!("cannot copy {}: {e}", binary.display()))?;

    let tarball = root
        .join("dist")
        .join("web")
        .join(format!("{BINARY}-{target}.tar.gz"));
    pack(&dist, &tarball)?;
    Ok(format!("dist-web: {}", tarball.display()))
}

fn run_tool(root: &Path, command: &mut Command) -> Result<(), String> {
    let shown = format!("{command:?}");
    println!("dist-web: {shown}");
    let status = command
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to run {shown}, is it on PATH? {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{shown} failed"))
}

/// Zig from `pip install ziglang`: cargo-zigbuild looks for `python3`, which on Windows can be just
/// the store's stub. A `zig` on PATH needs none of this.
fn zig_from_python() -> Option<String> {
    let on_path = Command::new("zig").arg("version").output();
    if on_path.is_ok_and(|out| out.status.success()) {
        return None;
    }
    let script = "import ziglang, os; \
                  print(os.path.join(os.path.dirname(ziglang.__file__), 'zig.exe' if os.name == 'nt' else 'zig'))";
    let out = Command::new("python").args(["-c", script]).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let entries = fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("cannot copy {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Unix permissions are set here, as a Windows file system has none to take over.
fn pack(dist: &Path, tarball: &Path) -> Result<(), String> {
    let file = File::create(tarball).map_err(|e| format!("cannot create tarball: {e}"))?;
    let mut archive = Builder::new(GzEncoder::new(file, Compression::default()));
    add_tree(&mut archive, dist, Path::new(""))?;
    archive
        .into_inner()
        .and_then(GzEncoder::finish)
        .map_err(|e| format!("cannot finish tarball: {e}"))?;
    Ok(())
}

fn add_tree(
    archive: &mut Builder<GzEncoder<File>>,
    dir: &Path,
    prefix: &Path,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = prefix.join(entry.file_name());
        if entry.path().is_dir() {
            add_tree(archive, &entry.path(), &name)?;
        } else {
            let data = fs::read(entry.path()).map_err(|e| e.to_string())?;
            let mut header = Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(mode_of(&name));
            // The host's `Last-Modified`: a fixed date would make browsers keep an old version.
            header.set_mtime(modified_seconds(&entry.path()));
            header.set_cksum();
            archive
                .append_data(&mut header, &name, data.as_slice())
                .map_err(|e| format!("cannot pack {}: {e}", name.display()))?;
        }
    }
    Ok(())
}

fn modified_seconds(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |age| age.as_secs())
}

fn mode_of(name: &Path) -> u32 {
    if name == Path::new(BINARY) {
        0o755
    } else {
        0o644
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tarball_has_forward_slashes_modes_and_real_dates() {
        let dir = tempfile::tempdir().unwrap();
        let dist = dir.path().join("dist");
        fs::create_dir_all(dist.join("public").join("assets")).unwrap();
        fs::write(dist.join(BINARY), "binary").unwrap();
        fs::write(dist.join("public").join("assets").join("a.js"), "js").unwrap();
        let tarball = dir.path().join("out.tar.gz");

        pack(&dist, &tarball).unwrap();

        let file = File::open(&tarball).unwrap();
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        let mut seen = Vec::new();
        for entry in archive.entries().unwrap() {
            let entry = entry.unwrap();
            let header = entry.header();
            assert!(header.mtime().unwrap() > 1_000_000_000, "a real date");
            seen.push((
                header.path().unwrap().to_string_lossy().into_owned(),
                header.mode().unwrap(),
            ));
        }
        seen.sort();
        assert_eq!(
            seen,
            [
                ("public/assets/a.js".to_string(), 0o644),
                (BINARY.to_string(), 0o755),
            ]
        );
    }

    #[test]
    fn a_target_cannot_leave_the_dist_directory() {
        for target in ["..", "../x", "a/b", "a\\b", "a..b"] {
            assert!(run(Path::new("."), target).is_err(), "{target}");
        }
    }

    #[test]
    fn only_the_binary_is_executable() {
        assert_eq!(mode_of(Path::new(BINARY)), 0o755);
        assert_eq!(mode_of(&Path::new("public").join("index.html")), 0o644);
        assert_eq!(mode_of(&Path::new("public").join(BINARY)), 0o644);
    }
}
