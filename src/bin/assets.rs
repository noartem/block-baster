//! Generates every icon (PWA icons, favicon and Android launcher icons) from
//! vector artwork defined in this file.
//! Usage: cargo run --release --bin assets

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() {
    desktop::main();
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod desktop {
    use std::fs;
    use std::path::{Path, PathBuf};

    use block_baster::color::Rgb;
    use block_baster::render::skia::{Ctx, alpha, linear, white};
    use tiny_skia::Pixmap;

    const COLORS: [&str; 7] = [
        "#ff6f61", "#ff9f1c", "#ffd23f", "#8bd346", "#41a6f6", "#8b5cf6", "#e85d9f",
    ];

    /// The logo: a small 3x3 board where an L-piece of blocks is landing, as
    /// (row, column, colour index).
    const LAYOUT: [(u32, u32, usize); 7] = [
        (0, 0, 0),
        (0, 1, 2),
        (1, 1, 2),
        (2, 0, 3),
        (2, 1, 4),
        (2, 2, 5),
        (0, 2, 6),
    ];

    /// Mixes a colour towards white (amount > 0) or black (amount < 0), in
    /// double precision like the old script, so the SVG text stays the same.
    fn mix(hex: &str, amount: f64) -> Rgb {
        let c = Rgb::hex(hex);
        let t = if amount > 0.0 { 255.0 } else { 0.0 };
        let a = amount.abs();
        let ch = |v: u8| {
            let v = v as f64;
            (v + (t - v) * a + 0.5).floor() as u8
        };
        Rgb::new(ch(c.r), ch(c.g), ch(c.b))
    }

    fn css(c: Rgb) -> String {
        format!("rgb({},{},{})", c.r, c.g, c.b)
    }

    /// `Math.round`.
    fn round(v: f64) -> f64 {
        (v + 0.5).floor()
    }

    // --- SVG text --------------------------------------------------------------
    // Numbers are formatted like JavaScript does (shortest round-trip form), so the
    // output matches the files the old Node script wrote.

    /// One bevelled block, same look as the in-game sprite.
    fn block_svg(x: f64, y: f64, s: f64, color: &str) -> String {
        let b = s * 0.15;
        let r = s * 0.1;
        let id = format!("c{}_{}", round(x), round(y));
        format!(
            r##"
  <g>
    <clipPath id="{id}"><rect x="{x}" y="{y}" width="{s}" height="{s}" rx="{r}"/></clipPath>
    <g clip-path="url(#{id})">
      <rect x="{x}" y="{y}" width="{s}" height="{s}" fill="{color}"/>
      <polygon points="{x},{y} {xs},{y} {xsb},{yb} {xb},{yb}" fill="{m1}"/>
      <polygon points="{x},{y} {xb},{yb} {xb},{ysb} {x},{ys}" fill="{m2}"/>
      <polygon points="{xs},{y} {xs},{ys} {xsb},{ysb} {xsb},{yb}" fill="{m3}"/>
      <polygon points="{x},{ys} {xb},{ysb} {xsb},{ysb} {xs},{ys}" fill="{m4}"/>
      <rect x="{xb}" y="{yb}" width="{inner}" height="{inner}" fill="{m5}"/>
      <ellipse cx="{cx}" cy="{cy}" rx="{erx}" ry="{ery}" fill="#fff" opacity="0.4"/>
    </g>
    <rect x="{x1}" y="{y1}" width="{s2}" height="{s2}" rx="{r}" fill="none" stroke="{m6}" stroke-width="{sw}"/>
  </g>"##,
            xs = x + s,
            ys = y + s,
            xsb = x + s - b,
            ysb = y + s - b,
            xb = x + b,
            yb = y + b,
            inner = s - 2.0 * b,
            cx = x + b + s * 0.16,
            cy = y + b + s * 0.13,
            erx = s * 0.15,
            ery = s * 0.09,
            x1 = x + 1.0,
            y1 = y + 1.0,
            s2 = s - 2.0,
            sw = s * 0.025,
            m1 = css(mix(color, 0.42)),
            m2 = css(mix(color, 0.2)),
            m3 = css(mix(color, -0.18)),
            m4 = css(mix(color, -0.32)),
            m5 = css(mix(color, 0.08)),
            m6 = css(mix(color, -0.45)),
        )
    }

    /// Where the logo's parts go: (art, o, cell).
    fn logo_geometry(size: f64, scale: f64) -> (f64, f64, f64) {
        let art = size * scale;
        let o = (size - art) / 2.0;
        (art, o, art / 3.0)
    }

    /// `scale` (0..1) controls how much of the canvas the artwork covers.
    fn logo_svg(size: f64, scale: f64, background: bool, round_k: f64) -> String {
        let (art, o, cell) = logo_geometry(size, scale);
        let blocks: String = LAYOUT
            .iter()
            .map(|&(r, c, k)| {
                block_svg(
                    o + c as f64 * cell + cell * 0.03,
                    o + r as f64 * cell + cell * 0.03,
                    cell * 0.94,
                    COLORS[k],
                )
            })
            .collect();
        let bg = if background {
            format!(
                r##"<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#22b0ac"/><stop offset="1" stop-color="#0b4f5c"/></linearGradient></defs>
       <rect width="{size}" height="{size}" rx="{rx}" fill="url(#bg)"/>
       <rect x="{bx}" y="{bx}" width="{bw}" height="{bw}" rx="{br}" fill="#0a2f38"/>"##,
                rx = size * round_k,
                bx = o - cell * 0.12,
                bw = art + cell * 0.24,
                br = cell * 0.25,
            )
        } else {
            String::new()
        };
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">{bg}{blocks}</svg>"#
        )
    }

    // --- Raster versions of the same artwork -----------------------------------

    /// An SVG `<rect rx>`: the radius is clamped to half the shorter side.
    fn svg_rect(ctx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, rx: f32) {
        ctx.round_rect(x, y, w, h, rx.min(w / 2.0).min(h / 2.0));
    }

    fn block(ctx: &mut Ctx, x: f64, y: f64, s: f64, color: &str) {
        let (x, y, s) = (x as f32, y as f32, s as f32);
        let base = Rgb::hex(color);
        let b = s * 0.15;
        let r = s * 0.1;
        ctx.save();
        svg_rect(ctx, x, y, s, s, r);
        ctx.clip();
        ctx.fill_rect(x, y, s, s, base);
        ctx.poly(
            &[x, y, x + s, y, x + s - b, y + b, x + b, y + b],
            mix(color, 0.42),
        );
        ctx.poly(
            &[x, y, x + b, y + b, x + b, y + s - b, x, y + s],
            mix(color, 0.2),
        );
        ctx.poly(
            &[
                x + s,
                y,
                x + s,
                y + s,
                x + s - b,
                y + s - b,
                x + s - b,
                y + b,
            ],
            mix(color, -0.18),
        );
        ctx.poly(
            &[
                x,
                y + s,
                x + b,
                y + s - b,
                x + s - b,
                y + s - b,
                x + s,
                y + s,
            ],
            mix(color, -0.32),
        );
        ctx.fill_rect(x + b, y + b, s - 2.0 * b, s - 2.0 * b, mix(color, 0.08));
        ctx.begin_path();
        ctx.ellipse(
            x + b + s * 0.16,
            y + b + s * 0.13,
            s * 0.15,
            s * 0.09,
            0.0,
            0.0,
            std::f32::consts::TAU,
            false,
        );
        ctx.fill(white(0.4));
        ctx.restore();
        ctx.set_line_width(s * 0.025);
        svg_rect(ctx, x + 1.0, y + 1.0, s - 2.0, s - 2.0, r);
        ctx.stroke(mix(color, -0.45));
    }

    fn background_gradient(size: f32) -> tiny_skia::Shader<'static> {
        linear(
            0.0,
            0.0,
            0.0,
            size,
            &[
                (0.0, alpha(Rgb::hex("#22b0ac"), 1.0)),
                (1.0, alpha(Rgb::hex("#0b4f5c"), 1.0)),
            ],
        )
    }

    /// Draws the logo in the coordinates of a `size`×`size` SVG.
    fn logo(ctx: &mut Ctx, size: f64, scale: f64, background: bool, round_k: f64) {
        let (art, o, cell) = logo_geometry(size, scale);
        if background {
            let s = size as f32;
            svg_rect(ctx, 0.0, 0.0, s, s, (size * round_k) as f32);
            ctx.fill(background_gradient(s));
            let (bx, bw) = ((o - cell * 0.12) as f32, (art + cell * 0.24) as f32);
            svg_rect(ctx, bx, bx, bw, bw, (cell * 0.25) as f32);
            ctx.fill("#0a2f38");
        }
        for &(r, c, k) in &LAYOUT {
            block(
                ctx,
                o + c as f64 * cell + cell * 0.03,
                o + r as f64 * cell + cell * 0.03,
                cell * 0.94,
                COLORS[k],
            );
        }
    }

    /// Renders artwork drawn for an `svg`×`svg` canvas at `px`×`px` pixels.
    fn icon(svg: f64, px: u32, draw: impl FnOnce(&mut Ctx)) -> Pixmap {
        let mut ctx = Ctx::new(px, px);
        ctx.scale(px as f32 / svg as f32, px as f32 / svg as f32);
        draw(&mut ctx);
        ctx.into_pixmap()
    }

    fn background_only(ctx: &mut Ctx, size: f32) {
        ctx.fill_rect(0.0, 0.0, size, size, background_gradient(size));
    }

    fn out(root: &Path, p: &str) -> PathBuf {
        let file = root.join(p);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        file
    }

    /// Writes a PNG with straight alpha and the best compression.
    fn png(pixmap: Pixmap, file: PathBuf) {
        let (w, h) = (pixmap.width(), pixmap.height());
        let data = pixmap.take_demultiplied();
        let mut enc = png::Encoder::new(
            std::io::BufWriter::new(fs::File::create(&file).unwrap()),
            w,
            h,
        );
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }

    pub fn main() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // --- Web / PWA -------------------------------------------------------------
        fs::write(
            out(&root, "public/favicon.svg"),
            logo_svg(64.0, 0.8, true, 0.22),
        )
        .unwrap();
        fs::write(
            out(&root, "assets/icon.svg"),
            logo_svg(1024.0, 0.72, true, 0.22),
        )
        .unwrap();
        png(
            icon(512.0, 512, |c| logo(c, 512.0, 0.72, true, 0.22)),
            out(&root, "public/pwa-512.png"),
        );
        png(
            icon(192.0, 192, |c| logo(c, 192.0, 0.72, true, 0.22)),
            out(&root, "public/pwa-192.png"),
        );
        png(
            icon(180.0, 180, |c| logo(c, 180.0, 0.72, true, 0.0)),
            out(&root, "public/apple-touch-icon.png"),
        );
        png(
            icon(512.0, 512, |c| logo(c, 512.0, 0.56, true, 0.0)),
            out(&root, "public/pwa-maskable-512.png"),
        );
        png(
            icon(1024.0, 1024, |c| logo(c, 1024.0, 0.72, true, 0.22)),
            out(&root, "assets/icon-1024.png"),
        );

        // --- Android ---------------------------------------------------------------
        let res = "android/app/src/main/res";
        if root.join(res).exists() {
            let densities = [
                ("mdpi", 1.0),
                ("hdpi", 1.5),
                ("xhdpi", 2.0),
                ("xxhdpi", 3.0),
                ("xxxhdpi", 4.0),
            ];
            for (d, k) in densities {
                let launcher = (48.0f64 * k).round() as u32;
                let adaptive = (108.0f64 * k).round() as u32;
                png(
                    icon(512.0, launcher, |c| logo(c, 512.0, 0.72, true, 0.22)),
                    out(&root, &format!("{res}/mipmap-{d}/ic_launcher.png")),
                );
                png(
                    icon(512.0, launcher, |c| logo(c, 512.0, 0.72, true, 0.5)),
                    out(&root, &format!("{res}/mipmap-{d}/ic_launcher_round.png")),
                );
                // Adaptive icon foreground: 108dp canvas, artwork inside the 66dp safe zone.
                png(
                    icon(432.0, adaptive, |c| logo(c, 432.0, 0.5, false, 0.22)),
                    out(
                        &root,
                        &format!("{res}/mipmap-{d}/ic_launcher_foreground.png"),
                    ),
                );
                png(
                    icon(432.0, adaptive, |c| background_only(c, 432.0)),
                    out(
                        &root,
                        &format!("{res}/mipmap-{d}/ic_launcher_background.png"),
                    ),
                );
            }
            println!("Android icons written.");
        }
        println!("Web assets written.");
    }
}
