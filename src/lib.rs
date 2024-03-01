//! Independently testable types and functions.

use serde::Deserialize;
use std::ops::Not;
use std::path::{Path, PathBuf};

/// The git forge in which a project's source code is stored.
pub enum GitHost {
    Github,
    Gitlab,
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
                "{}/releases/download/v$pkgver/{}-$pkgver-x86_64.tar.gz",
                package.repository, package.name
            ),
            GitHost::Gitlab => format!(
                "{}/-/archive/v$pkgver/{}-$pkgver-x86_64.tar.gz",
                package.repository, package.name
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
    #[serde(default)]
    pub metadata: Metadata,
}

impl Package {
    /// The name of the tarball that should be produced from this `Package`.
    pub fn tarball(&self, output: &Path) -> PathBuf {
        output.join(format!("{}-{}-x86_64.tar.gz", self.name, self.version))
    }

    /// The git host of this Package's repository.
    pub fn git_host(&self) -> GitHost {
        if self.repository.starts_with("https://gitlab") {
            GitHost::Gitlab
        } else {
            GitHost::Github
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Metadata {
    #[serde(default)]
    pub aur: Aur,
}


/// The inner values of a `[package.metadata.aur]` TOML block.
#[derive(Debug, Deserialize)]
pub struct Aur {
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub optdepends: Vec<String>,
    #[serde(default)]
    pub archive: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

impl Default for Aur {
    fn default() -> Aur {
        Aur{
            depends: Vec::new(),
            optdepends: Vec::new(),
            archive: None,
            name: None
        }
    }
}

impl std::fmt::Display for Aur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (deps, opts) = (self.depends.as_slice(), self.optdepends.as_slice());

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

