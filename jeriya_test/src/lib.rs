use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
};

use chrono::Utc;
use image::{codecs::png::PngEncoder, DynamicImage, ImageBuffer, ImageEncoder, ImageError, PixelWithColorType, Rgb, RgbImage};
use jeriya_shared::winit::{
    event_loop::EventLoopBuilder,
    platform::windows::EventLoopBuilderExtWindows,
    window::{Window, WindowBuilder},
};

/// Creates a new `TestContext` for the test function in which the macro is executed.
#[cfg(test)]
macro_rules! test_context {
    () => {{
        const TEST_RESULT_FOLDER: &str = "test_results";
        let test_name = jeriya_shared::function_name!().replace("::", ".").replace("jeriya_test.", "");
        TestContext::new(&test_name, &PathBuf::from(TEST_RESULT_FOLDER))
    }};
}

/// Create a winit window
pub fn create_window() -> Window {
    let event_loop = EventLoopBuilder::new().with_any_thread(true).build();
    WindowBuilder::new().with_visible(false).build(&event_loop).unwrap()
}

/// General information for a test
pub struct TestContext {
    pub test_name: String,
    pub debug_output_folder: PathBuf,
    pub general_debug_output_folder: PathBuf,
}

impl TestContext {
    /// Creates a new `TestContext` and determines a folder in the `general_debug_output_folder`
    /// to which the files for the test represented by this `TestContext` can be written.
    pub fn new(test_name: &str, general_debug_output_folder: &Path) -> Self {
        TestContext {
            test_name: test_name.to_owned(),
            debug_output_folder: general_debug_output_folder.join(test_name),
            general_debug_output_folder: general_debug_output_folder.to_path_buf(),
        }
    }
}

impl TestContext {
    /// Prepare folder on the filesystem
    pub fn prepare_output_folder(&self) {
        println!(
            "The files for debugging this test will be written to the following folder: {}",
            self.debug_output_folder.to_string_lossy()
        );
        fs::create_dir_all(&self.debug_output_folder).expect("Failed to create the debug_output_folder");
    }
}

/// Opens the given image and expects the path to be correct.
pub fn open_image(path: impl AsRef<Path>) -> DynamicImage {
    let f = |err: ImageError| {
        let err = err.to_string();
        let path_str = path
            .as_ref()
            .canonicalize()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(path.as_ref().to_string_lossy().into_owned());
        let cwd = PathBuf::from(".")
            .canonicalize()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or("unknown".to_owned());
        panic!("Could not find test image at path \"{path_str}\" (cwd: \"{cwd}\") due to the following error: {err}")
    };
    image::open(&path).unwrap_or_else(f)
}

/// Save the given image and expect the operation to succeed.
pub fn save_image(image: RgbImage, path: impl AsRef<Path>) {
    image
        .save(&path)
        .unwrap_or_else(|_| panic!("Failed to save image2 to path \"{}\"", path.as_ref().to_string_lossy()));
}

/// Uses the Reinhard tone mapping operator to produce an `ImageBuffer` that can be encoded as PNG.
pub fn reinhard_tone_map(image_buffer: &ImageBuffer<Rgb<f32>, Vec<f32>>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let width = image_buffer.width();
    let height = image_buffer.height();
    let data = image_buffer
        .clone()
        .into_vec()
        .into_iter()
        .map(|value| (255.0 * value / (1.0 + value)) as u8)
        .collect::<Vec<_>>();
    let image_buffer: ImageBuffer<Rgb<u8>, Vec<_>> =
        ImageBuffer::from_vec(width, height, data).expect("Failed to create the image buffer with casted values");
    image_buffer
}

/// Compare two images using a hybrid metric and assert that the score is between the `min` and `max` values.
pub fn assert_compare_hybrid(image1: RgbImage, image2: RgbImage, min: f64, max: f64, test_context: &TestContext) {
    assert!(min <= max, "min must be less or equal to max");

    let result = image_compare::rgb_hybrid_compare(&image1, &image2).expect("Images have different dimensions");

    let is_min_ok = result.score >= min;
    let is_max_ok = result.score <= max;
    if !is_min_ok || !is_max_ok {
        test_context.prepare_output_folder();

        // Write the two source images to the `debug_output_folder`
        save_image(image1, test_context.debug_output_folder.join("image1.png"));
        save_image(image2, test_context.debug_output_folder.join("image2.png"));

        // Convert the image containing f32 values into a u8 image using the a tone mapping operator
        let image_buffer = reinhard_tone_map(&result.image);

        // Write the image showing the diff to the `debug_output_folder`
        let formatted = Utc::now().format("%Y-%m-%d_%H-%M-%S-%f").to_string();
        let diff_path = test_context.debug_output_folder.join(format!("diff_{formatted}.png"));
        let file = File::create(diff_path).unwrap();
        let writer = BufWriter::new(file);
        let encoder = PngEncoder::new(writer);
        encoder
            .write_image(&image_buffer, result.image.width(), result.image.height(), Rgb::<u8>::COLOR_TYPE)
            .unwrap();
    }
    assert!(
        is_min_ok,
        "the compare score {} is less than the expected {} value",
        result.score, min
    );
    assert!(
        is_max_ok,
        "the compare score {} is greater than the expected {} value",
        result.score, max
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn image_not_found() {
        open_image("the/wrong/path/to/the/image");
    }

    #[test]
    fn einstein_compare_success() {
        let test_context = test_context!();
        let image1 = open_image("content/einstein-image004.jpg").into_rgb8();
        let image2 = open_image("content/einstein-image010.jpg").into_rgb8();
        assert_compare_hybrid(image1, image2, 0.82, 0.83, &test_context);
    }

    #[test]
    #[should_panic]
    fn einstein_compare_failure() {
        let test_context = test_context!();
        let image1 = open_image("content/einstein-image004.jpg").into_rgb8();
        let image2 = open_image("content/einstein-image010.jpg").into_rgb8();
        assert_compare_hybrid(image1, image2, 0.0, 0.0, &test_context);
    }
}
