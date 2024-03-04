//!	Package information structs, constants, and functions.

use serde::Deserialize;
use std::path::{Path,PathBuf};

///	Common licenses provided in the Arch Linux `licenses` package.
///
///	While the package contains additional licenses, we exclude the ones
///	unlikely to be used by Rust crates.
pub const LICENSES: [&str;14] = [
	"AGPL-3.0-only",
	"AGPL-3.0-or-later",
	"Apache-2.0",
	"BSL-1.0",				//	Boost Software License
	"GPL-2.0-only",
	"GPL-2.0-or-later",
	"GPL-3.0-only",
	"GPL-3.0-or-later",
	"LGPL-2.0-only",
	"LGPL-2.0-or-later",
	"LGPL-3.0-only",
	"LGBL-3.0-or-later",
	"MPL-2.0",				//	Mozilla Public License
	"Unlicense",			//	Not to be confused with "Unlicensed"
];

#[derive(Deserialize, Debug)]
pub struct Config {
	pub package: Package,
	#[serde(default)]
	bin: Vec<Binary>,
}

impl Config {
	///	The name of the main binary this project compiles to.
	pub fn binary_name(&self) -> &str {
		self.bin.first()
			.map(|bin| bin.name.as_str())
			.unwrap_or(self.package.name.as_str())
	}
}

#[derive(Deserialize, Debug)]
struct Binary {
	name: String
}

///	The relevant `Cargo.toml` fields to output a PKGBUILD.
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
	///	The name of the binary tarball this Package will produce.
	pub fn tarball(&self, output: &Path) -> PathBuf {
		output.join(format!("{}-{}-x86_64.tar.gz", self.name, self.version))
	}

	///	The git host of this package's repository.
	pub fn git_host(&self) -> GitHost {
		if self.repository.starts_with("https://gitlab") {
			GitHost::Gitlab
		} else {
			GitHost::Github
		}
	}
}

///	`[package.metadata]` TOML block, used to access `[package.metadata.aur]`.
#[derive(Debug, Default, Deserialize)]
pub struct Metadata {
	#[serde(default)]
	pub aur: Aur,
}

///	The values of a `[package.metadata.aur]` TOML block.
#[derive(Debug, Deserialize)]
pub struct Aur {
	#[serde(default)]
	pub archive: Option<String>,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub depends: Vec<String>,
	#[serde(default)]
	pub optdepends: Vec<String>,
}

impl Default for Aur {
	fn default() -> Aur {
		Aur{
			archive: None,
			name: None,
			depends: Vec::new(),
			optdepends: Vec::new(),
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
					write!(f, "\"{item}\"")?;
				}
				write!(f, "\"{last}\"{}", if !opts.is_empty() { "\n" } else { "" })?;
			}
			[] => {}
		}

		match opts {
			[middle @ .., last] => {
				write!(f, "optdepends=(")?;
				for item in middle {
					write!(f, "\"{item}\"")?;
				}
				write!(f, "\"{last}\"")?;
			}
			[] => {}
		}

		Ok(())
	}
}

pub enum GitHost {
	Github,
	Gitlab,
}

impl GitHost {
	///	The expected tarball location for a Package.
	pub fn source(&self, package: &Package) -> String {
		let path = std::env::var("CARGO_AUR_ARCHIVE").ok();
		if let Some(path) = path { return path; }

		let repository = &package.repository;
		let name = &package.name;

		match self {
			GitHost::Github => format!(
				"{repository}/releases/download/v$pkgver/{name}-$pkgver-x86_64.tar.gz"
			),
			GitHost::Gitlab => format!(
				"{repository}/-/archive/v$pkgver/{name}-$pkgver-x86_64.tar.gz"
			),
		}
	}
}


