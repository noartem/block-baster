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

struct AndroidHost {
    app: AndroidApp,
}

impl Host for AndroidHost {
    /// The status and navigation bars (and any display cutout) cover the edges of the window.
    fn insets(&self, window: &Window) -> Insets {
        let from_view = with_activity(|env, activity| {
            let win = env.call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
            let decor = env.call_method(&win, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
            let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
            if insets.is_null() {
                return Ok(None);
            }
            let mut side = |name: &str| env.call_method(&insets, name, "()I", &[]).and_then(|v| v.i()).map(|v| v.max(0) as f32);
            Ok(Some(Insets {
                top: side("getSystemWindowInsetTop")?,
                right: side("getSystemWindowInsetRight")?,
                bottom: side("getSystemWindowInsetBottom")?,
                left: side("getSystemWindowInsetLeft")?,
            }))
        })
        .flatten()
        .unwrap_or_default();
        // The content rect tells the same when the window isn't drawn edge to edge.
        let size = window.inner_size();
        let rect = self.app.content_rect();
        let from_rect = Insets {
            top: rect.top.max(0) as f32,
            left: rect.left.max(0) as f32,
            right: (size.width as i32 - rect.right).max(0) as f32,
            bottom: (size.height as i32 - rect.bottom).max(0) as f32,
        };
        Insets {
            top: from_view.top.max(from_rect.top),
            right: from_view.right.max(from_rect.right),
            bottom: from_view.bottom.max(from_rect.bottom),
            left: from_view.left.max(from_rect.left),
        }
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("BlockBaster"));
    let dir = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("."));
    let store = FileStore::open(dir.join("storage.json"));
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
