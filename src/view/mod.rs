//! Everything drawn each frame with femtovg (OpenGL ES 3 / WebGL2).

pub mod background;
pub mod draw;
pub mod renderer;
pub mod scene;
pub mod text;
pub mod text_cache;
pub mod ui;

use femtovg::{ImageFlags, ImageId, ImageSource, renderer::OpenGl};

pub type Canvas = femtovg::Canvas<OpenGl>;

/// Uploads a tiny-skia image (premultiplied RGBA) as a texture.
pub fn upload(c: &mut Canvas, pm: &tiny_skia::Pixmap) -> Option<ImageId> {
    use femtovg::rgb::FromSlice;
    let img = femtovg::imgref::Img::new(pm.data().as_rgba(), pm.width() as usize, pm.height() as usize);
    c.create_image(ImageSource::Rgba(img), ImageFlags::PREMULTIPLIED).ok()
}
