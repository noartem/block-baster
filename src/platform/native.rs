//! The window, OpenGL context and event loop for the desktop and Android
//! (winit + glutin), driving the platform-independent [`App`].

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use femtovg::renderer::OpenGl;
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::app::{App, PointerKind};
use crate::view::Canvas;
use crate::view::ui::Insets;

/// Platform hooks the shell needs besides the [`App`]'s own.
pub trait Host {
    /// Safe-area insets of the window in physical pixels.
    fn insets(&self, _window: &Window) -> Insets {
        Insets::default()
    }
    /// Options for tools: write a screenshot to this path once the screen settles, then quit.
    fn capture(&self) -> Option<std::path::PathBuf> {
        None
    }
}

struct Gl {
    window: Window,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
}

pub struct Shell<H: Host> {
    app: App,
    host: H,
    attributes: WindowAttributes,
    config: Option<Config>,
    /// Kept while the app is in the background (Android drops the window).
    paused_context: Option<NotCurrentContext>,
    gl: Option<Gl>,
    canvas: Option<Canvas>,
    started: Instant,
    mouse: (f32, f32),
    mouse_down: bool,
    scale: f64,
    last_frame: f64,
    frames: u32,
    /// Insets of the last resize, and when they were last checked again.
    insets: Insets,
    insets_checked: f64,
}

impl<H: Host> Shell<H> {
    pub fn new(app: App, host: H, attributes: WindowAttributes) -> Self {
        Shell {
            app,
            host,
            attributes,
            config: None,
            paused_context: None,
            gl: None,
            canvas: None,
            started: Instant::now(),
            mouse: (0.0, 0.0),
            mouse_down: false,
            scale: 1.0,
            last_frame: 0.0,
            frames: 0,
            insets: Insets::default(),
            insets_checked: 0.0,
        }
    }

    /// Milliseconds since start: the app's frame clock.
    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    pub fn run(mut self, event_loop: EventLoop<()>) {
        if let Err(e) = event_loop.run_app(&mut self) {
            eprintln!("event loop error: {e}");
        }
        self.app.save();
    }

    fn create_gl(&mut self, el: &ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
        // femtovg needs a stencil buffer for concave shapes.
        let template = ConfigTemplateBuilder::new().with_stencil_size(8).with_alpha_size(8);
        let (window, config) = match &self.config {
            Some(config) => (glutin_winit::finalize_window(el, self.attributes.clone(), config)?, config.clone()),
            None => {
                let builder = DisplayBuilder::new().with_window_attributes(Some(self.attributes.clone()));
                let (window, config) = builder.build(el, template, |configs| {
                    configs.reduce(|best, c| if c.stencil_size() >= 8 && (best.stencil_size() < 8 || c.num_samples() < best.num_samples()) { c } else { best }).expect("an OpenGL config")
                })?;
                (window.ok_or("no window")?, config)
            }
        };
        let display = config.display();
        let context = match self.paused_context.take() {
            Some(c) => c,
            None => {
                let handle = window.window_handle()?.as_raw();
                let attempts = [
                    ContextAttributesBuilder::new().with_context_api(ContextApi::Gles(Some(Version::new(3, 0)))).build(Some(handle)),
                    ContextAttributesBuilder::new().build(Some(handle)),
                    ContextAttributesBuilder::new().with_context_api(ContextApi::Gles(None)).build(Some(handle)),
                ];
                let mut made = None;
                for attrs in &attempts {
                    if let Ok(c) = unsafe { display.create_context(&config, attrs) } {
                        made = Some(c);
                        break;
                    }
                }
                made.ok_or("could not create an OpenGL context")?
            }
        };
        let surface_attrs = window.build_surface_attributes(Default::default())?;
        let surface = unsafe { display.create_window_surface(&config, &surface_attrs)? };
        let context = context.make_current(&surface)?;
        let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).expect("1")));
        if self.canvas.is_none() {
            let renderer = unsafe { OpenGl::new_from_function_cstr(|s| display.get_proc_address(s).cast()) }?;
            let canvas = Canvas::new(renderer)?;
            self.canvas = Some(canvas);
        }
        self.config = Some(config);
        self.scale = window.scale_factor();
        self.gl = Some(Gl { window, surface, context });
        self.resize();
        Ok(())
    }

    fn resize(&mut self) {
        let Some(gl) = &self.gl else { return };
        let size = gl.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        gl.window.resize_surface(&gl.surface, &gl.context);
        if let Some(canvas) = &mut self.canvas {
            canvas.set_size(size.width, size.height, 1.0);
        }
        let s = self.scale as f32;
        let i = self.host.insets(&gl.window);
        self.insets = i;
        let insets = Insets { top: i.top / s, right: i.right / s, bottom: i.bottom / s, left: i.left / s };
        self.app.resize(size.width as f32 / s, size.height as f32 / s, s, insets);
        gl.window.request_redraw();
    }

    fn draw(&mut self, el: &ActiveEventLoop) {
        let now = self.now();
        let (Some(gl), Some(canvas)) = (&self.gl, &mut self.canvas) else { return };
        let size = gl.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        canvas.reset();
        canvas.clear_rect(0, 0, size.width, size.height, femtovg::Color::black());
        canvas.reset_transform();
        canvas.scale(self.scale as f32, self.scale as f32);
        self.app.frame(canvas, now);
        canvas.flush();
        self.frames += 1;
        if let Some(path) = self.host.capture()
            && self.frames > 5
            && !self.app.needs_frame()
        {
            if let Ok(img) = canvas.screenshot() {
                save_png(&path, img.width() as u32, img.height() as u32, img.buf());
            }
            el.exit();
        }
        if let Err(e) = gl.surface.swap_buffers(&gl.context) {
            eprintln!("swap_buffers: {e}");
        }
        self.last_frame = now;
    }

    fn pointer(&self, x: f64, y: f64) -> (f32, f32) {
        ((x / self.scale) as f32, (y / self.scale) as f32)
    }
}

impl<H: Host> ApplicationHandler for Shell<H> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gl.is_none()
            && let Err(e) = self.create_gl(el)
        {
            eprintln!("OpenGL setup failed: {e}");
            el.exit();
            return;
        }
        self.app.set_visible(true);
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        self.app.set_visible(false);
        // Android takes the window away: keep the context (and with it every texture) for later.
        if let Some(gl) = self.gl.take() {
            match gl.context.make_not_current() {
                Ok(c) => self.paused_context = Some(c),
                Err(e) => eprintln!("make_not_current: {e}"),
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.app.save();
                el.exit();
            }
            WindowEvent::Resized(_) => self.resize(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor;
                self.resize();
            }
            WindowEvent::RedrawRequested => self.draw(el),
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = self.pointer(position.x, position.y);
                self.mouse = (x, y);
                if self.mouse_down {
                    self.app.pointer_move(0, x, y);
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = self.mouse;
                match state {
                    ElementState::Pressed => {
                        self.mouse_down = true;
                        self.app.pointer_down(0, PointerKind::Mouse, x, y);
                    }
                    ElementState::Released => {
                        self.mouse_down = false;
                        self.app.pointer_up(0, x, y);
                    }
                }
            }
            WindowEvent::CursorLeft { .. } if self.mouse_down => {
                // Keep dragging: the release outside the window arrives as a normal MouseInput.
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => -(p.y / self.scale) as f32,
                };
                self.app.wheel(dy);
            }
            WindowEvent::Touch(touch) => {
                let (x, y) = self.pointer(touch.location.x, touch.location.y);
                let id = touch.id + 1;
                match touch.phase {
                    TouchPhase::Started => self.app.pointer_down(id, PointerKind::Touch, x, y),
                    TouchPhase::Moved => self.app.pointer_move(id, x, y),
                    TouchPhase::Ended => self.app.pointer_up(id, x, y),
                    TouchPhase::Cancelled => self.app.pointer_cancel(id),
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed && !event.repeat => match event.logical_key {
                Key::Named(NamedKey::Escape) => self.app.key_escape(),
                Key::Named(NamedKey::BrowserBack | NamedKey::GoBack) => self.app.back_button(),
                _ => {}
            },
            WindowEvent::Focused(false) => {
                // A release while another window has focus may never arrive.
                self.mouse_down = false;
                self.app.cancel_input();
                self.app.save();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = self.now();
        self.app.tick(now);
        // System bars can settle after the window appears (Android reports them late).
        if now - self.insets_checked > 1000.0
            && let Some(gl) = &self.gl
        {
            self.insets_checked = now;
            if self.host.insets(&gl.window) != self.insets {
                self.resize();
            }
        }
        let Some(gl) = &self.gl else {
            el.set_control_flow(ControlFlow::Wait);
            return;
        };
        if self.app.needs_frame() {
            gl.window.request_redraw();
            el.set_control_flow(ControlFlow::Wait);
            return;
        }
        let mut wake: Option<f64> = self.app.next_wakeup();
        if self.app.background_animates() {
            // Animated backgrounds run at 30 fps while nothing else moves.
            let next = self.last_frame + 33.0;
            if now >= next {
                gl.window.request_redraw();
            } else {
                wake = Some(wake.map_or(next, |w| w.min(next)));
            }
        }
        match wake {
            Some(at) => el.set_control_flow(ControlFlow::WaitUntil(self.started + Duration::from_secs_f64(at.max(now) / 1000.0))),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.app.save();
    }
}

/// Writes RGBA pixels (top row first) to a PNG file.
#[cfg(not(target_os = "android"))]
pub fn save_png(path: &std::path::Path, width: u32, height: u32, pixels: &[femtovg::rgb::RGBA8]) {
    use femtovg::rgb::ComponentBytes;
    let file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(e) => return eprintln!("{}: {e}", path.display()),
    };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    match enc.write_header().and_then(|mut w| w.write_image_data(pixels.as_bytes())) {
        Ok(()) => println!("wrote {}", path.display()),
        Err(e) => eprintln!("{}: {e}", path.display()),
    }
}

#[cfg(target_os = "android")]
pub fn save_png(_path: &std::path::Path, _width: u32, _height: u32, _pixels: &[femtovg::rgb::RGBA8]) {}
