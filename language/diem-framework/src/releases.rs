// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use move_binary_format::file_format::CompiledModule;
use move_command_line_common::files::MOVE_ERROR_DESC_EXTENSION;
use move_package::{
    compilation::compiled_package::{CompiledPackage, OnDiskCompiledPackage},
    source_package::manifest_parser::parse_move_manifest_from_file,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ReleaseFetcher {
    package_dir_name: String,
    release_name: String,
}

impl ReleaseFetcher {
    pub fn new(package_dir_name: &str, release_name: &str) -> Self {
        Self {
            package_dir_name: package_dir_name.to_string(),
            release_name: release_name.to_string(),
        }
    }

    /// Load the serialized modules from the specified release.
    pub fn module_blobs(&self) -> Result<Vec<Vec<u8>>> {
        Ok(self
            .modules()?
            .into_iter()
            .map(|module| {
                let mut bytes = vec![];
                module.serialize(&mut bytes).unwrap();
                bytes
            })
            .collect())
    }

    pub fn modules(&self) -> Result<Vec<CompiledModule>> {
        Ok(self
            .package()?
            .transitive_compiled_modules()
            .compute_dependency_graph()
            .compute_topological_order()?
            .into_iter()
            .map(Clone::clone)
            .collect())
    }

    /// Load the serialized modules from the specified release.
    pub fn package(&self) -> Result<CompiledPackage> {
        let root_path = Path::new(std::env!("CARGO_MANIFEST_DIR")).join(&self.package_dir_name);
        let package_name = parse_move_manifest_from_file(&root_path)?.package.name;
        let path = root_path
            .join("releases")
            .join("artifacts")
            .join(&self.release_name)
            .join("build")
            .join(package_name.as_str());
        Ok(OnDiskCompiledPackage::from_path(&path)
            .unwrap()
            .into_compiled_package()
            .unwrap())
    }

    pub fn error_descriptions(&self) -> Result<Vec<u8>> {
        let path = Path::new(std::env!("CARGO_MANIFEST_DIR"))
            .join(&self.package_dir_name)
            .join("releases")
            .join("artifacts")
            .join(&self.release_name)
            .join("error_description")
            .join("error_description")
            .with_extension(MOVE_ERROR_DESC_EXTENSION);
        Ok(std::fs::read(&path)?)
    }
}
