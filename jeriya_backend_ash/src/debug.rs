use std::{
    ffi::CStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use ash::vk;
use jeriya_shared::log::{error, info, warn};

use crate::{entry::Entry, instance::Instance, Result};

static PANIC_ON_MESSAGE: AtomicBool = AtomicBool::new(true);

/// Determines whether a panic should be raised when the validation layer emits a message
pub fn set_panic_on_message(value: bool) {
    PANIC_ON_MESSAGE.store(value, Ordering::SeqCst);
}

/// Represents the callback of the validation layer
pub struct ValidationLayerCallback {
    messenger: vk::DebugUtilsMessengerEXT,
    _instance: Arc<Instance>,
    _entry: Arc<Entry>,
}

impl Drop for ValidationLayerCallback {
    fn drop(&mut self) {
        unsafe {
            // The DebugUtils are only set in the instance when the validation layer is enabled
            self._instance
                .debug_utils
                .as_ref()
                .expect("DebugUtils must be Some when Dropping the ValidationLayerCallback")
                .destroy_debug_utils_messenger(self.messenger, None)
        };
    }
}

impl ValidationLayerCallback {
    /// Sets up the validation layer callback that logs the validation layer messages
    pub fn new(entry: &Arc<Entry>, instance: &Arc<Instance>) -> Result<ValidationLayerCallback> {
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
            flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            pfn_user_callback: Some(debug_utils_messenger_callback),
            ..Default::default()
        };
        let messenger = unsafe {
            // The DebugUtils are only set in the instance when the validation layer is enabled
            instance
                .debug_utils
                .as_ref()
                .expect("DebugUtils must be Some when Dropping the ValidationLayerCallback")
                .create_debug_utils_messenger(&create_info, None)?
        };
        Ok(ValidationLayerCallback {
            messenger,
            _entry: entry.clone(),
            _instance: instance.clone(),
        })
    }
}

unsafe extern "system" fn debug_utils_messenger_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let types = {
        let mut types = Vec::new();
        if message_types.contains(vk::DebugUtilsMessageTypeFlagsEXT::GENERAL) {
            types.push("General");
        }
        if message_types.contains(vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE) {
            types.push("Performance");
        }
        if message_types.contains(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION) {
            types.push("Validation");
        }
        if message_types.is_empty() {
            panic!("Unknown message type");
        }
        types.join(", ")
    };

    let message = {
        let message = CStr::from_ptr((*p_callback_data).p_message)
            .to_str()
            .expect("failed to convert validation layer message to str")
            .replace("[ VUID", "\n\t[ VUID")
            .replace("Object 0", "\n\tObject 0")
            .replace("Object 1", "\n\tObject 1")
            .replace("Object 2", "\n\tObject 2")
            .replace('|', "\n\t|")
            .replace("The Vulkan spec states:", "\n\tThe Vulkan spec states:")
            .replace("(https", "\n\t(https");

        format!("[ValidationLayer] [{types}] {message}\n")
    };

    let write_function = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => |m| info!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => |m| warn!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => |m| info!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => |m| error!("{m}"),
        _ => panic!("Unhandled severity \"{message_severity:?}\"; message: {message}"),
    };

    let is_ok = matches!(
        message_severity,
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
    );
    if PANIC_ON_MESSAGE.load(Ordering::SeqCst) && !is_ok {
        panic!("{}", message);
    } else {
        write_function(message);
    }

    vk::FALSE
}

#[cfg(test)]
mod tests {
    mod debug_utils_messenger_callback {
        use std::ffi::c_void;

        use super::super::*;

        #[test]
        #[should_panic]
        fn panic() {
            set_panic_on_message(true);
            let data = vk::DebugUtilsMessengerCallbackDataEXT {
                p_message: b"my_message\n\0".as_ptr() as *const i8,
                ..Default::default()
            };
            unsafe {
                debug_utils_messenger_callback(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
                    &data as *const vk::DebugUtilsMessengerCallbackDataEXT,
                    std::ptr::null::<()>() as *mut c_void,
                );
            }
        }

        #[test]
        fn smoke() {
            set_panic_on_message(false);
            let data = vk::DebugUtilsMessengerCallbackDataEXT {
                p_message: b"my_message\n\0".as_ptr() as *const i8,
                ..Default::default()
            };
            unsafe {
                debug_utils_messenger_callback(
                    vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
                    &data as *const vk::DebugUtilsMessengerCallbackDataEXT,
                    std::ptr::null::<()>() as *mut c_void,
                );
            }
        }
    }
}
