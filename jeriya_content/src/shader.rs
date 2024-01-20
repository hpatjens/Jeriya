use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::asset_processor::AssetBuilder;

pub struct ShaderAsset {
    name: String,
    spriv: Vec<u8>,
}

impl ShaderAsset {
    pub fn compile_from(src_path: impl AsRef<Path>, dst_path: impl AsRef<Path>) -> crate::Result<Self> {
        let src_path_str = src_path
            .as_ref()
            .to_str()
            .ok_or(crate::Error::InvalidPath(src_path.as_ref().to_path_buf()))?;
        let dst_path_str = dst_path
            .as_ref()
            .to_str()
            .ok_or(crate::Error::InvalidPath(dst_path.as_ref().to_path_buf()))?;
        let name = src_path
            .as_ref()
            .file_name()
            .ok_or(crate::Error::InvalidPath(PathBuf::from(src_path_str)))?
            .to_str()
            .ok_or(crate::Error::InvalidPath(PathBuf::from(src_path_str)))?
            .to_string();

        // Execute the shader compiler
        let output = Command::new("glslc.exe")
            .args(&[src_path_str, "-o", dst_path_str])
            .output()
            .map_err(|error| crate::Error::FailedToCompileShader(format!("Could not execute shader compiler: {}", error)))?;

        // Check if the shader compiler was successful
        let stdout = String::from_utf8(output.stdout).map_err(|error| crate::Error::Utf8Error(error))?;
        let stderr = String::from_utf8(output.stderr).map_err(|error| crate::Error::Utf8Error(error))?;

        if output.status.success() {
            let spirv = std::fs::read(dst_path_str).map_err(|error| crate::Error::IoError(error))?;
            Ok(Self::new(name, spirv))
        } else {
            let message = format!(
                "Shader compiler exited with exit code {}:\nstdout:\n{}\nstderr:\n{}",
                output.status.code().expect("code must be set when unsuccessful"),
                stdout,
                stderr
            );
            Err(crate::Error::FailedToCompileShader(message))
        }
    }

    /// Creates a new shader from SPIR-V bytecode.
    pub fn new(name: impl Into<String>, spriv: Vec<u8>) -> Self {
        Self { name: name.into(), spriv }
    }

    /// Returns the name of the shader.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the SPIR-V bytecode of the shader.
    pub fn spriv(&self) -> &[u8] {
        &self.spriv
    }
}

/// Processes a model asset.
pub fn process_shader(asset_builder: &mut AssetBuilder) -> crate::Result<()> {
    let dst_path = asset_builder.processed_asset_path().join("shader.spv");
    asset_builder.with_file(&dst_path);
    ShaderAsset::compile_from(asset_builder.unprocessed_asset_path(), dst_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use jeriya_shared::function_name;
    use jeriya_test::create_test_result_folder_for_function;

    use super::*;

    #[test]
    fn test_compile_shader() {
        let src_path = "test_data/test.vert";
        let dst_path = create_test_result_folder_for_function(function_name!()).join("test.vert.spv");
        let shader = ShaderAsset::compile_from(src_path, &dst_path).unwrap();
        assert!(!shader.spriv().is_empty());
        assert_eq!(shader.name(), "test.vert");
    }
}
