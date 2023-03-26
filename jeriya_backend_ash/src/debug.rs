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
            .expect("failed to convert validation layer message to str");
        format!("[DebugUtils] [{types}] {message}")
    };

    let write_function = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => |m| info!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => |m| warn!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => |m| info!("{m}"),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => |m| error!("{m}"),
        _ => panic!("Unhandled severity \"{message_severity:?}\"; message: {message}"),
    };

    if PANIC_ON_MESSAGE.load(Ordering::SeqCst) {
        panic!("{}", message);
    } else {
        write_function(message);
    }

    vk::FALSE
}
