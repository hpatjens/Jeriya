use jeriya_shared::features;

use crate::command_buffer_builder::CommandBufferBuilder;

pub fn label_color_red(value: f32) -> [f32; 4] {
    [value, 0.0, 0.0, 1.0]
}

/// Makes sure that a debug label is correctly ended.
pub struct DebugLabelGuard {
    label: &'static str,
    correctly_dropped: bool,
}

impl DebugLabelGuard {
    // Creates a new guard to ensure that a debug label is correctly ended.
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            // When labeling is enabled, then the guard must be dropped correctly. When it is disabled, then the guard is not needed and therefore always correctly dropped.
            correctly_dropped: !features::LABELING,
        }
    }

    /// Ends the label scope. This must be called or the `LabelGuard` will panic on drop.
    pub fn end(mut self, command_buffer_builder: &mut CommandBufferBuilder) {
        command_buffer_builder.end_label_scope();
        self.correctly_dropped = true;
    }
}

impl Drop for DebugLabelGuard {
    fn drop(&mut self) {
        // The drop implementation doesn't directly drop the label because it would have to store a reference to the `CommandBufferBuilder` which is mostly uses &mut self and therefore would be exclusively borrowed until the label is dropped.
        let name = self.label;
        if !self.correctly_dropped {
            panic!("LabelGuard for '{name}' was not dropped correctly");
        }
    }
}
