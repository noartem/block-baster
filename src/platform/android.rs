//! The Android app: a NativeActivity running the winit shell. Vibration,
//! leaving the app, opening links and the system bar insets go through JNI.

use jni::objects::{JObject, JValue};
use jni::{JNIEnv, JavaVM};
use winit::event_loop::EventLoop;
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::platform::android::activity::AndroidApp;
use winit::window::Window;

use super::native::{Host, Shell};
use crate::app::{App, Platform, Strength};
use crate::storage::FileStore;
use crate::view::ui::Insets;

/// Runs `f` with a JNI environment and the activity; exceptions are cleared and turn into `None`.
fn with_activity<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    // A global reference owned by the NativeActivity; never deleted here.
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    let result = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return None;
    }
    result.map_err(|e| log::warn!("JNI: {e}")).ok()
}

fn vibrator<'a>(env: &mut JNIEnv<'a>, activity: &JObject) -> jni::errors::Result<JObject<'a>> {
    let name = env.new_string("vibrator")?;
    env.call_method(activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&name)])?.l()
}

struct Android {
    can_vibrate: bool,
}

impl Platform for Android {
    fn can_vibrate(&self) -> bool {
        self.can_vibrate
    }

    fn vibrate(&mut self, strength: Strength, intensity: f32) {
        if intensity <= 0.0 {
            return;
        }
        // A stronger setting moves each kick up a level, like the three haptic styles before.
        let scale = intensity * 2.0;
        let index = match strength {
            Strength::Light => 0.0,
            Strength::Medium => 1.0,
            Strength::Heavy => 2.0,
        };
        let level = (((index + 1.0) * scale).round() as i32 - 1).clamp(0, 2);
        let (ms, amplitude) = [(12i64, 70i32), (20, 150), (35, 255)][level as usize];
        with_activity(|env, activity| {
            let vib = vibrator(env, activity)?;
            let effect = env.call_static_method("android/os/VibrationEffect", "createOneShot", "(JI)Landroid/os/VibrationEffect;", &[JValue::Long(ms), JValue::Int(amplitude)])?.l()?;
            env.call_method(&vib, "vibrate", "(Landroid/os/VibrationEffect;)V", &[JValue::Object(&effect)])?;
            Ok(())
        });
    }

    fn is_app(&self) -> bool {
        true
    }

    fn exit(&mut self) {
        with_activity(|env, activity| env.call_method(activity, "finish", "()V", &[]).map(|_| ()));
    }

    fn open_url(&mut self, url: &str) {
        with_activity(|env, activity| {
            let text = env.new_string(url)?;
            let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&text)])?.l()?;
            let action = env.new_string("android.intent.action.VIEW")?;
            let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&action), JValue::Object(&uri)])?;
            env.call_method(activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
            Ok(())
        });
    }
}

/// Reads the four `int` sides of an object, by getter (`WindowInsets`) or by field (`android.graphics.Insets`).
fn sides(env: &mut JNIEnv, obj: &JObject, names: [&str; 4], fields: bool) -> jni::errors::Result<Insets> {
    let mut side = |name: &str| {
        let value = if fields { env.get_field(obj, name, "I") } else { env.call_method(obj, name, "()I", &[]) };
        value.and_then(|v| v.i()).map(|v| v.max(0) as f32)
    };
    Ok(Insets { top: side(names[0])?, right: side(names[1])?, bottom: side(names[2])?, left: side(names[3])? })
}

/// The system bars and cutout as the window manager has them (Android 11+). Unlike the
/// view's insets, this doesn't depend on the UI thread having laid the window out yet.
fn metrics_insets() -> Option<Insets> {
    if AndroidApp::sdk_version() < 30 {
        return None;
    }
    with_activity(|env, activity| {
        let wm = env.call_method(activity, "getWindowManager", "()Landroid/view/WindowManager;", &[])?.l()?;
        let metrics = env.call_method(&wm, "getCurrentWindowMetrics", "()Landroid/view/WindowMetrics;", &[])?.l()?;
        let insets = env.call_method(&metrics, "getWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        let bars = env.call_static_method("android/view/WindowInsets$Type", "systemBars", "()I", &[])?.i()?;
        let cutout = env.call_static_method("android/view/WindowInsets$Type", "displayCutout", "()I", &[])?.i()?;
        let both = env.call_method(&insets, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(bars | cutout)])?.l()?;
        sides(env, &both, ["top", "right", "bottom", "left"], true)
    })
}

/// The insets the decor view last received; `None` until it has been laid out.
fn view_insets() -> Option<Insets> {
    with_activity(|env, activity| {
        let win = env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&win, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        if insets.is_null() {
            return Ok(None);
        }
        let names = ["getSystemWindowInsetTop", "getSystemWindowInsetRight", "getSystemWindowInsetBottom", "getSystemWindowInsetLeft"];
        sides(env, &insets, names, false).map(Some)
    })
    .flatten()
}

/// The status bar's height from the system resources, for when nothing else knows the insets.
fn status_bar_insets() -> Option<Insets> {
    with_activity(|env, activity| {
        let res = env.call_method(activity, "getResources", "()Landroid/content/res/Resources;", &[])?.l()?;
        let (name, kind, package) = (env.new_string("status_bar_height")?, env.new_string("dimen")?, env.new_string("android")?);
        let args = [JValue::Object(&name), JValue::Object(&kind), JValue::Object(&package)];
        let id = env.call_method(&res, "getIdentifier", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I", &args)?.i()?;
        if id == 0 {
            return Ok(None);
        }
        let top = env.call_method(&res, "getDimensionPixelSize", "(I)I", &[JValue::Int(id)])?.i()?;
        Ok(Some(Insets { top: top.max(0) as f32, ..Insets::default() }))
    })
    .flatten()
}

fn max_insets(a: Insets, b: Insets) -> Insets {
    Insets { top: a.top.max(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom), left: a.left.max(b.left) }
}

struct AndroidHost {
    app: AndroidApp,
}

impl Host for AndroidHost {
    /// The status and navigation bars (and any display cutout) cover the edges of the window.
    fn insets(&self, window: &Window) -> Insets {
        // The content rect tells the same when the window isn't drawn edge to edge.
        let size = window.inner_size();
        let rect = self.app.content_rect();
        let from_rect = Insets {
            top: rect.top.max(0) as f32,
            left: rect.left.max(0) as f32,
            right: (size.width as i32 - rect.right).max(0) as f32,
            bottom: (size.height as i32 - rect.bottom).max(0) as f32,
        };
        let (metrics, view) = (metrics_insets(), view_insets());
        // Some devices report nothing from either, at least at first; the status bar is always shown.
        let fallback = if metrics.is_none() && view.is_none() { status_bar_insets() } else { None };
        [metrics, view, fallback].into_iter().flatten().fold(from_rect, max_insets)
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("BlockBaster"));
    let dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut store = FileStore::open(dir.join("storage.json"));
    // The data directory is `files/`; the WebView of the earlier app kept its data next to it.
    if let Some(app_dir) = dir.parent() {
        let empty = store.is_empty();
        let moved = super::legacy::migrate(&mut store, empty, &app_dir.join(super::legacy::WEBVIEW_STORAGE), &dir.join("legacy-tmp"));
        if moved > 0 {
            log::info!("carried over {moved} saved items from the WebView version");
        }
    }
    let can_vibrate = with_activity(|env, activity| {
        let vib = vibrator(env, activity)?;
        env.call_method(&vib, "hasVibrator", "()Z", &[])?.z()
    })
    .unwrap_or(false);
    let event_loop = match EventLoop::builder().with_android_app(app.clone()).build() {
        Ok(el) => el,
        Err(e) => return log::error!("event loop: {e}"),
    };
    let game = App::new(Box::new(store), Box::new(Android { can_vibrate }), None, 0.0);
    Shell::new(game, AndroidHost { app }, Window::default_attributes()).run(event_loop);
}
