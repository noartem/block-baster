//! Dev tool: renders contact sheets of every block look and board background,
//! to check the procedural graphics against the original web version.
//! Usage: cargo run --release --bin sprite_sheet [-- OUT_DIR [CELL DPR]]

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() {
    desktop::main();
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod desktop {
    use std::path::PathBuf;
    use std::time::Instant;

    use block_baster::game::board::SIZE;
    use block_baster::render::board::board_background;
    use block_baster::render::sprites::{LOOKS, block_sprite};
    use block_baster::styles::{Style, TIERS, color_for};
    use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

    const GAP: u32 = 8;

    fn blit(dst: &mut Pixmap, src: &Pixmap, x: u32, y: u32) {
        dst.draw_pixmap(
            x as i32,
            y as i32,
            src.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    pub fn main() {
        let out = PathBuf::from(
            std::env::args()
                .nth(1)
                .unwrap_or_else(|| "target/sprite-sheet".into()),
        );
        std::fs::create_dir_all(&out).expect("create output dir");

        // Sprites: one row per tier, the 7 block colours + grey at 64 px, then at 128 px,
        // on the tier's (first variant's) board colour.
        let sizes = [64u32, 128];
        let row_h = 128 + GAP * 2;
        let width = GAP + sizes.iter().map(|s| 8 * (s + GAP)).sum::<u32>() + GAP;
        let mut sheet = Pixmap::new(width, row_h * TIERS.len() as u32).unwrap();
        for (i, tier) in TIERS.iter().enumerate() {
            let palette = &tier.variants[0];
            let b = palette.board;
            let row = tiny_skia::Rect::from_xywh(
                0.0,
                (i as u32 * row_h) as f32,
                width as f32,
                row_h as f32,
            )
            .unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(Color::from_rgba8(b.r, b.g, b.b, 255));
            sheet.fill_rect(row, &paint, Transform::identity(), None);
            let mut x = GAP;
            for &px in &sizes {
                for id in [1u8, 2, 3, 4, 5, 6, 7, 0] {
                    let sprite = block_sprite(tier.block, color_for(id, palette), px, 0);
                    blit(
                        &mut sheet,
                        &sprite,
                        x,
                        i as u32 * row_h + GAP + (128 - px) / 2,
                    );
                    x += px + GAP;
                }
                x += GAP;
            }
        }
        sheet.save_png(out.join("sprites.png")).unwrap();

        // Looks: per tier, every look of four colours at 96 px, in board order.
        let px = 96u32;
        let per = LOOKS as u32 * px + GAP * 2;
        let mut looks = Pixmap::new(GAP + 4 * per, (px + GAP * 2) * TIERS.len() as u32).unwrap();
        for (i, tier) in TIERS.iter().enumerate() {
            let palette = &tier.variants[0];
            let b = palette.board;
            let row = tiny_skia::Rect::from_xywh(0.0, (i as u32 * (px + GAP * 2)) as f32, looks.width() as f32, (px + GAP * 2) as f32).unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(Color::from_rgba8(b.r, b.g, b.b, 255));
            looks.fill_rect(row, &paint, Transform::identity(), None);
            for (k, id) in [1u8, 2, 3, 5].into_iter().enumerate() {
                for look in 0..LOOKS {
                    let sprite = block_sprite(tier.block, color_for(id, palette), px, look);
                    blit(&mut looks, &sprite, GAP + k as u32 * per + look as u32 * px, i as u32 * (px + GAP * 2) + GAP);
                }
            }
        }
        looks.save_png(out.join("looks.png")).unwrap();

        // Board backgrounds: one image per tier with its three variants side by side.
        let arg = |i: usize, default: f32| {
            std::env::args()
                .nth(i)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let (cell, dpr) = (arg(2, 48.0), arg(3, 2.0));
        let board = cell * SIZE as f32;
        for (i, tier) in TIERS.iter().enumerate() {
            let imgs: Vec<Pixmap> = tier
                .variants
                .iter()
                .enumerate()
                .map(|(variant, palette)| {
                    board_background(
                        &Style {
                            tier,
                            palette,
                            variant,
                        },
                        cell,
                        board,
                        dpr,
                    )
                })
                .collect();
            let w = imgs[0].width();
            let mut strip = Pixmap::new(w * 3, w).unwrap();
            // A mid-grey backdrop shows the glows and translucent frames.
            strip.fill(Color::from_rgba8(60, 60, 70, 255));
            for (k, img) in imgs.iter().enumerate() {
                blit(&mut strip, img, k as u32 * w, 0);
            }
            strip.save_png(out.join(format!("board-{i}.png"))).unwrap();
        }

        // Timings (meaningful in release builds).
        let runs = 20;
        for tier in TIERS.iter() {
            let t = Instant::now();
            for _ in 0..runs {
                block_sprite(tier.block, color_for(1, &tier.variants[0]), 150, 0);
            }
            println!(
                "sprite {:?} 150px: {:.2} ms",
                tier.block,
                t.elapsed().as_secs_f64() * 1000.0 / runs as f64
            );
        }
        for tier in TIERS.iter() {
            let s = Style {
                tier,
                palette: &tier.variants[0],
                variant: 0,
            };
            let t = Instant::now();
            for _ in 0..5 {
                board_background(&s, 60.0, 480.0, 3.0);
            }
            println!(
                "board {:?} cell 60 dpr 3: {:.1} ms",
                tier.frame,
                t.elapsed().as_secs_f64() * 1000.0 / 5.0
            );
        }
        println!("written to {}", out.display());
    }
}
