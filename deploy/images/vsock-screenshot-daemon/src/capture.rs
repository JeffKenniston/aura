use std::error::Error;
use std::fmt;
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
use x11rb::rust_connection::RustConnection;

#[derive(Debug)]
pub enum CaptureError {
    X11Connection(String),
    X11Protocol(String),
    Encoding(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::X11Connection(msg) => write!(f, "X11 connection error: {}", msg),
            CaptureError::X11Protocol(msg) => write!(f, "X11 protocol error: {}", msg),
            CaptureError::Encoding(msg) => write!(f, "Image encoding error: {}", msg),
        }
    }
}

impl Error for CaptureError {}

/// Captures the current X11 root window on `display_name` (e.g., ":0") and returns PNG bytes.
pub fn capture_screen(display_name: Option<&str>) -> Result<Vec<u8>, CaptureError> {
    // Attempt connection with up to 5 retries in case Xvfb is spinning up
    let mut last_err = String::new();
    let mut conn_opt = None;
    let mut screen_num = 0;

    for attempt in 1..=5 {
        match x11rb::connect(display_name) {
            Ok((conn, s_num)) => {
                conn_opt = Some(conn);
                screen_num = s_num;
                break;
            }
            Err(e) => {
                last_err = format!("{}", e);
                if attempt < 5 {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    let conn: RustConnection = conn_opt.ok_or_else(|| {
        CaptureError::X11Connection(format!(
            "Failed to connect to X11 display '{:?}': {}",
            display_name, last_err
        ))
    })?;

    let setup = conn.setup();
    let screen = setup
        .roots
        .get(screen_num)
        .ok_or_else(|| CaptureError::X11Protocol("Invalid screen index".to_string()))?;

    let root = screen.root;
    let geom = conn
        .get_geometry(root)
        .map_err(|e| CaptureError::X11Protocol(format!("Failed to get root geometry: {}", e)))?
        .reply()
        .map_err(|e| CaptureError::X11Protocol(format!("Reply error for geometry: {}", e)))?;

    let width = geom.width;
    let height = geom.height;

    if width == 0 || height == 0 {
        return Err(CaptureError::X11Protocol(
            "Screen dimensions are zero".to_string(),
        ));
    }

    let img = conn
        .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)
        .map_err(|e| CaptureError::X11Protocol(format!("Failed to get image: {}", e)))?
        .reply()
        .map_err(|e| CaptureError::X11Protocol(format!("Reply error for image: {}", e)))?;

    let raw_data = &img.data;
    let pixel_count = (width as usize) * (height as usize);
    let mut rgba_data = Vec::with_capacity(pixel_count * 4);

    // Determine bits per pixel from raw data length
    let bytes_per_pixel = if pixel_count > 0 {
        raw_data.len() / pixel_count
    } else {
        4
    };

    match bytes_per_pixel {
        4 => {
            // Standard 32-bpp (24-bit depth padded): Little Endian is BGRA / BGR0
            for chunk in raw_data.chunks_exact(4) {
                let b = chunk[0];
                let g = chunk[1];
                let r = chunk[2];
                let a = 255u8;
                rgba_data.extend_from_slice(&[r, g, b, a]);
            }
        }
        3 => {
            // Packed 24-bpp: BGR
            for chunk in raw_data.chunks_exact(3) {
                let b = chunk[0];
                let g = chunk[1];
                let r = chunk[2];
                let a = 255u8;
                rgba_data.extend_from_slice(&[r, g, b, a]);
            }
        }
        _ => {
            // Fallback: convert whatever we have or zero-pad
            for (i, chunk) in raw_data.chunks(bytes_per_pixel.max(1)).enumerate() {
                if i >= pixel_count {
                    break;
                }
                let b = *chunk.first().unwrap_or(&0u8);
                let g = *chunk.get(1).unwrap_or(&0u8);
                let r = *chunk.get(2).unwrap_or(&0u8);
                rgba_data.extend_from_slice(&[r, g, b, 255u8]);
            }
        }
    }

    encode_png(width as u32, height as u32, &rgba_data)
}

/// Encodes raw RGBA buffer into standard PNG byte stream.
pub fn encode_png(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, CaptureError> {
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder
            .write_header()
            .map_err(|e| CaptureError::Encoding(format!("Failed to write PNG header: {}", e)))?;

        writer
            .write_image_data(rgba_data)
            .map_err(|e| CaptureError::Encoding(format!("Failed to encode image data: {}", e)))?;
    }

    Ok(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_png_dummy_buffer() {
        let width = 10;
        let height = 10;
        let mut dummy_rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            dummy_rgba.extend_from_slice(&[255, 0, 0, 255]); // red
        }

        let png_res = encode_png(width, height, &dummy_rgba);
        assert!(png_res.is_ok());
        let png_bytes = png_res.unwrap();
        // Check PNG signature: 89 50 4E 47 0D 0A 1A 0A
        assert_eq!(&png_bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }
}
