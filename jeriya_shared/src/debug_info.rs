use std::{borrow::Cow, time::Instant};

/// Returns the [`DebugInfo`] of a value.
pub trait AsDebugInfo {
    fn as_debug_info(&self) -> &DebugInfo;
}

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
    pub ptr: Option<u64>,
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

    pub fn with_ptr(mut self, ptr: u64) -> Self {
        self.ptr = Some(ptr);
        self
    }

    pub fn format_one_line(&self) -> String {
        let name = format!("{:?}", self.name);
        format!("DebugInfo {{ name: {name} }}")
    }
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
    ($name:literal, $value:expr) => {
        $crate::DebugInfo::default()
            .with_name(std::borrow::Cow::Borrowed($name))
            .with_origin_function_name($crate::function_name!())
            .with_code_location($crate::code_location!())
            .with_created_now()
            .with_ptr($value)
    };
    ($name:expr, $value:expr) => {
        $crate::DebugInfo::default()
            .with_name(std::borrow::Cow::Owned($name.to_string()))
            .with_origin_function_name($crate::function_name!())
            .with_code_location($crate::code_location!())
            .with_created_now()
            .with_ptr($value)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    mod debug_info {
        use crate::function_name;

        use super::*;

        #[test]
        fn with_name_borrowed() {
            DebugInfo::default().with_name(Cow::Borrowed("my_texture"));
        }

        #[test]
        fn with_name_owned() {
            DebugInfo::default().with_name(Cow::Owned("my_texture".to_owned()));
        }

        #[test]
        fn with_created_now() {
            DebugInfo::default().with_created_now();
        }

        #[test]
        fn format_one_line_with_name() {
            let line = DebugInfo::default().with_name(Cow::Borrowed("my_texture")).format_one_line();
            assert_eq!(line, "DebugInfo { name: Some(\"my_texture\") }");
        }

        #[test]
        fn format_one_line_without_name() {
            let line = DebugInfo::default().format_one_line();
            assert_eq!(line, "DebugInfo { name: None }");
        }

        #[test]
        fn all() {
            // Given
            let instant = Instant::now();
            let value = Box::new(12);

            // When
            let debug_info = DebugInfo::default()
                .with_name(Cow::Borrowed("my_texture"))
                .with_origin_function_name(function_name!())
                .with_code_location(CodeLocation { file: "main.rs", line: 1 })
                .with_created_instant(instant)
                .with_ptr(value.as_ref() as *const i32 as u64);

            // Then
            assert_eq!(debug_info.name.unwrap(), Cow::Borrowed("my_texture"));
            assert_eq!(debug_info.created_instant.unwrap(), instant);
            assert_eq!(
                debug_info.origin_function_name.unwrap(),
                "jeriya_shared::debug_info::tests::debug_info::all"
            );
            assert_eq!(debug_info.code_location.clone().unwrap().file, "main.rs");
            assert_eq!(debug_info.code_location.clone().unwrap().line, 1);
            assert_eq!(debug_info.ptr.unwrap(), value.as_ref() as *const i32 as u64);
        }
    }
}
