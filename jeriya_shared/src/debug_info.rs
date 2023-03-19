use std::{borrow::Cow, time::Instant};

/// Indicates where the something happened in the code.
#[derive(Debug, Clone)]
pub struct CodeLocation {
    pub file: &'static str,
    pub line: u32,
}

/// A set of information that can be attached to values helping the developer to debug the code.
#[derive(Default, Debug, Clone)]
pub struct DebugInfo {
    pub name: Option<Cow<'static, str>>,
    pub origin_function_name: Option<&'static str>,
    pub code_location: Option<CodeLocation>,
    pub created_instant: Option<Instant>,
}

impl DebugInfo {
    pub fn with_name(mut self, name: Cow<'static, str>) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_origin_function_name(mut self, origin_function_name: impl Into<&'static str>) -> Self {
        self.origin_function_name = Some(origin_function_name.into());
        self
    }

    pub fn with_code_location(mut self, code_location: CodeLocation) -> Self {
        self.code_location = Some(code_location);
        self
    }

    pub fn with_created_instant(mut self, created_instant: Instant) -> Self {
        self.created_instant = Some(created_instant);
        self
    }

    pub fn with_created_now(mut self) -> Self {
        self.created_instant = Some(Instant::now());
        self
    }

    pub fn format_one_line(&self) -> String {
        let name = self
            .name
            .as_ref()
            .map(|n| format!("name: {}", n))
            .unwrap_or_else(|| "unknown".to_owned());
        format!("DebugInfo {{ name: {name} }}")
    }
}

#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }};
}

#[macro_export]
macro_rules! code_location {
    () => {
        $crate::CodeLocation {
            file: file!(),
            line: line!(),
        }
    };
}

#[macro_export]
macro_rules! debug_info {
    ($name:literal) => {
        $crate::DebugInfo::default()
            .with_name(std::borrow::Cow::Borrowed($name))
            .with_origin_function_name($crate::function_name!())
            .with_code_location($crate::code_location!())
            .with_created_now()
    };
    ($name:expr) => {
        $crate::DebugInfo::default()
            .with_name(std::borrow::Cow::Owned($name.to_string()))
            .with_origin_function_name($crate::function_name!())
            .with_code_location($crate::code_location!())
            .with_created_now()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    mod debug_info {
        use super::*;

        #[test]
        fn borrowed_name() {
            DebugInfo::default().with_name(Cow::Borrowed("my_texture"));
        }

        #[test]
        fn owned_name() {
            DebugInfo::default().with_name(Cow::Owned("my_texture".to_owned()));
        }

        #[test]
        fn all() {
            // Given
            let instant = Instant::now();

            // When
            let debug_info = DebugInfo::default()
                .with_name(Cow::Borrowed("my_texture"))
                .with_origin_function_name(function_name!())
                .with_code_location(CodeLocation { file: "main.rs", line: 1 })
                .with_created_instant(instant);

            // Then
            assert_eq!(debug_info.name.unwrap(), Cow::Borrowed("my_texture"));
            assert_eq!(debug_info.created_instant.unwrap(), instant);
            assert_eq!(
                debug_info.origin_function_name.unwrap(),
                "jeriya_shared::debug_info::tests::debug_info::all"
            );
            assert_eq!(debug_info.code_location.clone().unwrap().file, "main.rs");
            assert_eq!(debug_info.code_location.clone().unwrap().line, 1);
        }
    }
}
