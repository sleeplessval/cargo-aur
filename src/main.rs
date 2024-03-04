mod config;
mod error;

use crate::config::{LICENSES, Config, Package};
use crate::error::Error;
use colored::*;
use gumdrop::{Options, ParsingStyle};
use hmac_sha256::Hash;
use std::fs::{DirEntry, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Options)]
struct Args {
    /// Display this help message.
    help: bool,
    /// Display the current version of this software.
    version: bool,
    /// Set a custom output directory (default: target/).
    output: Option<PathBuf>,
    /// Use the MUSL build target to produce a static binary.
    musl: bool,
    /// Don't actually build anything.
    dryrun: bool,
    /// collect unused free arg(s) so "cargo aur" doesn't panic
    #[options(free)]
    _free: Vec<String>
}

fn main() -> ExitCode {
    // Parse arguments
    let args = Args::parse_args_or_exit(ParsingStyle::AllOptions);

    // Handle version flag
    if args.version {
        let version = env!("CARGO_PKG_VERSION");
        println!("{}", version);
        return ExitCode::SUCCESS;
    }

    // Do actual work and handle the Result
    let result = work(args);
    if let Err(e) = result {
        eprintln!(
            "{} {}: {e}",
            "::".bold(),
            "Error".bold().red()
        );
        return ExitCode::FAILURE;
    }
    println!(
        "{} {}",
        "::".bold(),
        "Done.".bold().green()
    );
    ExitCode::SUCCESS
}

/// Main program body, wrapped for error handling.
fn work(args: Args) -> Result<(), Error> {
    // We can't proceed if the user has specified `--musl` but doesn't have the
    // target installed.
    if args.musl {
        p("Checking for musl toolchain...".bold());
        musl_check()?
    }

    // Where cargo expects to read and write to. By default we want to read the
    // built binary from `target/release` and we want to write our results to
    // `target/cargo-aur`, but these are configurable by the user.
    let cargo_target: PathBuf = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("target"),
    };

    let output = args.output.unwrap_or(cargo_target.join("cargo-aur"));

    // Ensure the target can actually be written to. Otherwise the `tar`
    // operation later on will fail.
    std::fs::create_dir_all(&output)?;

    // Read config from Cargo.toml
    let config = cargo_config()?;

    // Copy license file if needed
    let license = if must_copy_license(&config.package.license) {
        p("LICENSE file will be installed manually.".bold().yellow());
        Some(license_file()?)
    } else {
        None
    };

    // Handle dry-run flag
    if args.dryrun {
        return Ok(());
    }

    release_build(args.musl)?;
    tarball(args.musl, &cargo_target, &output, license.as_ref(), &config)?;
    let sha256: String = sha256sum(&config.package, &output)?;

    // Write the PKGBUILD.
    let path = output.join("PKGBUILD");
    let file = BufWriter::new(File::create(&path)?);
    pkgbuild(file, &config, &sha256, license.as_ref())?;

    Ok(())
}

/// Read the `Cargo.toml` for all the fields of concern to this tool.
fn cargo_config() -> Result<Config, Error> {
    // NOTE 2023-11-27 Yes it looks silly to be reading the whole thing into a
    // string here, but the `toml` library doesn't allow deserialization from
    // anything else but a string.
    let content = std::fs::read_to_string("Cargo.toml")?;
    let proj: Config = toml::from_str(&content)?;
    Ok(proj)
}

/// If a AUR package's license isn't included in `/usr/share/licenses/common/`,
/// then it must be installed manually by the PKGBUILD. MIT and BSD3 are such
/// missing licenses, and since many Rust crates use them we must make this
/// check.
fn must_copy_license(license: &str) -> bool {
    !LICENSES.contains(&license)
}

/// The path to the `LICENSE` file.
fn license_file() -> Result<DirEntry, Error> {
    std::fs::read_dir(".")?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with("LICENSE"))
                .unwrap_or(false)
        })
        .ok_or(Error::MissingLicense)
}

/// Write a legal PKGBUILD to some `Write` instance (a `File` in this case).
fn pkgbuild<T>(
    mut file: T,
    config: &Config,
    sha256: &str,
    license: Option<&DirEntry>,
) -> Result<(), Error>
where
    T: Write,
{
    let package = &config.package;
    let authors = package
        .authors
        .iter()
        .map(|a| format!("# Maintainer: {}", a))
        .collect::<Vec<_>>()
        .join("\n");

    // Pull fields from metadata
    let metadata = &package.metadata.aur;
    let package_name = metadata.name.clone()
        .unwrap_or( format!("{}-bin", package.name) );
    let source = metadata.archive.clone()
        .unwrap_or( package.git_host().source(&config.package) );
    let dependencies = format!("{}", metadata);

    // Write PKGBUILD
    writeln!(file, "{}", authors)?;
    writeln!(file, "#")?;
    writeln!(
        file,
        "# This PKGBUILD was generated by `cargo aur`: https://crates.io/crates/cargo-aur"
    )?;
    writeln!(file)?;
    writeln!(file, "pkgname={}", package_name)?;
    writeln!(file, "pkgver={}", package.version)?;
    writeln!(file, "pkgrel=1")?;
    writeln!(file, "pkgdesc=\"{}\"", package.description)?;
    writeln!(file, "url=\"{}\"", package.homepage)?;
    writeln!(file, "license=(\"{}\")", package.license)?;
    writeln!(file, "arch=(\"x86_64\")")?;
    writeln!(file, "provides=(\"{}\")", package.name)?;
    writeln!(file, "conflicts=(\"{}\")", package.name)?;

    if dependencies.len() > 0 {
        writeln!(file, "{}", metadata)?;
    }

    // If source property is not a URL, make it relative to the repository
    if !source.starts_with("https://") {
        writeln!(file, "source=(\"{}/{}\")", package.repository, source)?;
    } else {
        writeln!(file, "source=(\"{}\")", source)?;
    }
    writeln!(file, "sha256sums=(\"{}\")", sha256)?;
    writeln!(file)?;
    writeln!(file, "package() {{")?;
    writeln!(
        file,
        "    install -Dm755 {} -t \"$pkgdir/usr/bin\"",
        config.binary_name()
    )?;

    if let Some(lic) = license {
        let file_name = lic
            .file_name()
            .into_string()
            .map_err(|_| Error::Utf8OsString)?;
        writeln!(
            file,
            "    install -Dm644 {} \"$pkgdir/usr/share/licenses/$pkgname/{}\"",
            file_name, file_name
        )?;
    }

    writeln!(file, "}}")?;
    Ok(())
}

/// Run `cargo build --release`, potentially building statically.
fn release_build(musl: bool) -> Result<(), Error> {
    let mut args = vec!["build", "--release"];

    if musl {
        args.push("--target=x86_64-unknown-linux-musl");
    }

    p("Running release build...".bold());
    Command::new("cargo").args(args).status()?;
    Ok(())
}

fn tarball(
    musl: bool,
    cargo_target: &Path,
    output: &Path,
    license: Option<&DirEntry>,
    config: &Config,
) -> Result<(), Error> {
    let release_dir = if musl {
        "x86_64-unknown-linux-musl/release"
    } else {
        "release"
    };

    let binary_name = config.binary_name();
    let binary = cargo_target.join(release_dir).join(binary_name);

    strip(&binary)?;
    std::fs::copy(binary, binary_name)?;

    // Create the tarball.
    p("Packing tarball...".bold());
    let mut command = Command::new("tar");
    command
        .arg("czf")
        .arg(config.package.tarball(output))
        .arg(binary_name);
    if let Some(lic) = license {
        command.arg(lic.path());
    }
    command.status()?;

    std::fs::remove_file(binary_name)?;

    Ok(())
}

/// Strip the release binary, so that we aren't compressing more bytes than we
/// need to.
fn strip(path: &Path) -> Result<(), Error> {
    p("Stripping binary...".bold());
    Command::new("strip").arg(path).status()?;
    Ok(()) // FIXME Would love to use my `void` package here and elsewhere.
}

fn sha256sum(package: &Package, output: &Path) -> Result<String, Error> {
    let bytes = std::fs::read(package.tarball(output))?;
    let digest = Hash::hash(&bytes);
    let hex = digest.iter().map(|u| format!("{:02x}", u)).collect();
    Ok(hex)
}

/// Does the user have the `x86_64-unknown-linux-musl` target installed?
fn musl_check() -> Result<(), Error> {
    let args = ["target", "list", "--installed"];
    let output = Command::new("rustup").args(args).output()?.stdout;

    std::str::from_utf8(&output)?
        .lines()
        .any(|tc| tc == "x86_64-unknown-linux-musl")
        .then_some(())
        .ok_or(Error::MissingMuslTarget)
}

fn p(msg: ColoredString) {
    println!("{} {}", "::".bold(), msg)
}
