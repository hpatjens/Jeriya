use std::{
    ffi::CStr,
    sync::atomic::{AtomicBool, Ordering},
};

use ash::{extensions::ext::DebugUtils, vk, Entry, Instance};
use jeriya_shared::log::{error, info, warn};

use crate::Result;

static PANIC_ON_MESSAGE: AtomicBool = AtomicBool::new(true);

/// Determines whether a panic should be raised when the validation layer emits a message
pub fn set_panic_on_message(value: bool) {
    PANIC_ON_MESSAGE.store(value, Ordering::SeqCst);
}

/// Sets up the validation layer callback that logs the validation layer messages
pub fn setup_debug_utils(entry: &Entry, instance: &Instance) -> Result<()> {
    let debug_utils = DebugUtils::new(&entry, &instance);
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
    unsafe {
        debug_utils.create_debug_utils_messenger(&create_info, None)?;
    }
    Ok(())
}

unsafe extern "system" fn debug_utils_messenger_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message_type = match message_types {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "General",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "Performance",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "Validation",
        _ => panic!("Unknown message type"),
    };
    let message = format!("[DebugUtils] [{}] {:?}", message_type, CStr::from_ptr((*p_callback_data).p_message));
    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => info!("{}", message),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", message),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => info!("{}", message),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => panic!("{}", message),
        _ => {
            let message = format!(
                "Unhandled severity \"{:?}\" in DebugUtils callback. Message: {}",
                message_severity, message
            );
            if PANIC_ON_MESSAGE.load(Ordering::SeqCst) {
                panic!("{}", message);
            } else {
                error!("{}", message);
            }
        }
    }
    vk::FALSE
}
