//! Independently testable types and functions.

use serde::Deserialize;
use std::ops::Not;
use std::path::{Path, PathBuf};

/// The git forge in which a project's source code is stored.
pub enum GitHost {
    Github,
    Gitlab,
    Generic,
}

impl GitHost {
    /// The expected tarball location for a Package.
    pub fn source(&self, package: &Package) -> String {
        let path = std::env::var("CARGO_AUR_ARCHIVE").ok();
        if path.is_some() {
            return path.unwrap();
        }

        match self {
            GitHost::Github => format!(
                "{}/archive/refs/tags/v$pkgver.tar.gz",
                package.repository
            ),
            GitHost::Gitlab => format!(
                "{}/-/archive/v$pkgver/{}-$pkgver.tar.gz",
                package.repository, package.name
            ),
            GitHost::Generic => format!(
                "{}/archive/v$pkgver.tar.gz",
                package.repository
            ),
        }
    }
}

/// The critical fields read from a `Cargo.toml` and rewritten into a PKGBUILD.
#[derive(Deserialize, Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub description: String,
    pub homepage: String,
    pub repository: String,
    pub license: String,
    pub metadata: Option<Metadata>,
}

impl Package {

    /// The git host of this Package's repository.
    pub fn git_host(&self) -> GitHost {
        if self.repository.starts_with("https://github") {
            GitHost::Github
        } else if self.repository.starts_with("https://gitlab") {
            GitHost::Gitlab
        } else {
            GitHost::Generic
        }
    }
}

// {
//     Package {
//         name: "aura".to_string(),
//         version: "1.2.3".to_string(),
//         authors: vec![],
//         description: "".to_string(),
//         homepage: "".to_string(),
//         repository: "".to_string(),
//         license: "".to_string(),
//         metadata: None,
//     }.tarball(Path::new("foobar"))
// }

/// The `[package.metadata]` TOML block.
#[derive(Deserialize, Debug)]
pub struct Metadata {
    /// Deprecated.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Deprecated.
    #[serde(default)]
    pub optdepends: Vec<String>,
    /// > [package.metadata.aur]
    pub aur: Option<AUR>,
}

impl std::fmt::Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Reconcile which section to read extra dependency information from.
        // The format we hope the user is using is:
        //
        // > [package.metadata.aur]
        //
        // But version 1.5 originally supported:
        //
        // > [package.metadata]
        //
        // To avoid a sudden breakage for users, we support both definition
        // locations but favour the newer one.
        //
        // We print a warning to the user elsewhere if they're still using the
        // old way.
        let (deps, opts) = if let Some(aur) = self.aur.as_ref() {
            (aur.depends.as_slice(), aur.optdepends.as_slice())
        } else {
            (self.depends.as_slice(), self.optdepends.as_slice())
        };

        match deps {
            [middle @ .., last] => {
                write!(f, "depends=(")?;
                for item in middle {
                    write!(f, "\"{}\" ", item)?;
                }
                if opts.is_empty().not() {
                    writeln!(f, "\"{}\")", last)?;
                } else {
                    write!(f, "\"{}\")", last)?;
                }
            }
            [] => {}
        }

        match opts {
            [middle @ .., last] => {
                write!(f, "optdepends=(")?;
                for item in middle {
                    write!(f, "\"{}\" ", item)?;
                }
                write!(f, "\"{}\")", last)?;
            }
            [] => {}
        }

        Ok(())
    }
}

/// The inner values of a `[package.metadata.aur]` TOML block.
#[derive(Deserialize, Debug)]
pub struct AUR {
    #[serde(default)]
    depends: Vec<String>,
    #[serde(default)]
    optdepends: Vec<String>,
}
