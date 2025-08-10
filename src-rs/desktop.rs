use serde::de::DeserializeOwned;
use swift_rs::SRString;
use tauri::{ipc::Channel, plugin::PluginApi, AppHandle, Manager, Runtime};

use serde::Serialize;
use serde_json::Value as JsonValue;

use memoffset::offset_of;

use std::{
  collections::HashMap,
  fmt,
  sync::{mpsc::channel, Mutex, OnceLock},
};

use std::sync::atomic::{AtomicI32, Ordering};

use std::sync::Arc;

type PluginResponse = Result<serde_json::Value, serde_json::Value>;

type PendingPluginCallHandler = Box<dyn FnOnce(PluginResponse) + Send + 'static>;

static PENDING_PLUGIN_CALLS_ID: AtomicI32 = AtomicI32::new(0);
static PENDING_PLUGIN_CALLS: OnceLock<Mutex<HashMap<i32, PendingPluginCallHandler>>> =
  OnceLock::new();
static CHANNELS: OnceLock<Mutex<HashMap<u32, Channel<serde_json::Value>>>> = OnceLock::new();

/// Error response from the Kotlin and Swift backends.
#[derive(Debug, thiserror::Error, Clone, serde::Deserialize)]
pub struct ErrorResponse<T = ()> {
  /// Error code.
  pub code: Option<String>,
  /// Error message.
  pub message: Option<String>,
  /// Optional error data.
  #[serde(flatten)]
  pub data: T,
}

impl<T> fmt::Display for ErrorResponse<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(code) = &self.code {
      write!(f, "[{code}]")?;
      if self.message.is_some() {
        write!(f, " - ")?;
      }
    }
    if let Some(message) = &self.message {
      write!(f, "{message}")?;
    }
    Ok(())
  }
}

/// Possible errors when invoking a plugin.
#[derive(Debug, thiserror::Error)]
pub enum PluginInvokeError {
  /// Failed to reach platform webview handle.
  #[error("the webview is unreachable")]
  UnreachableWebview,
  /// Error returned from direct mobile plugin invoke.
  #[error(transparent)]
  InvokeRejected(#[from] ErrorResponse),
  /// Failed to deserialize response.
  #[error("failed to deserialize response: {0}")]
  CannotDeserializeResponse(serde_json::Error),
  /// Failed to serialize request payload.
  #[error("failed to serialize payload: {0}")]
  CannotSerializePayload(serde_json::Error),
}

#[repr(C)]
pub struct PluginApiRef<R: Runtime, C: DeserializeOwned> {
  handle: AppHandle<R>,
  name: &'static str,
  raw_config: Arc<JsonValue>,
  config: C,
}

#[repr(C)]
pub struct PluginApiExt<R: Runtime, C: DeserializeOwned>(PluginApi<R, C>);

impl<R: Runtime, C: DeserializeOwned> From<PluginApi<R, C>> for PluginApiExt<R, C> {
    fn from(api: PluginApi<R, C>) -> Self {
        PluginApiExt(api)
    }
}

impl<R: Runtime, C: DeserializeOwned> PluginApiExt<R, C> {
  /// Returns the app handle.
  pub fn app(&self) -> &AppHandle<R> {
    self.0.app()
  }

  /// Returns the plugin name.
  pub fn name(&self) -> &str {    
    let self_ptr = &self.0 as *const PluginApi<R, C> as *const u8;
    let offset = offset_of!(PluginApiRef<R, C>, name);
    
    let name: &'static str = unsafe {
        let field_ptr = self_ptr.add(offset) as *const &'static str;
        *field_ptr
    };
    name
  }

  /// Returns the raw plugin configuration.
  pub fn raw_config(&self) -> Arc<JsonValue> {
    let self_ptr = self as *const PluginApiExt<R, C> as *const u8;
    let offset = offset_of!(PluginApiRef<R, C>, raw_config);

    let rc_ptr = unsafe { self_ptr.add(offset) as *const Arc<JsonValue> };
    let rc_ref = unsafe { &*rc_ptr };
    rc_ref.clone()
  }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
impl<R: Runtime, C: DeserializeOwned> PluginApiExt<R, C> {
  /// Registers a Swift plugin.
  pub fn register_swift_plugin(
    &self,
    init_fn: unsafe fn() -> *const std::ffi::c_void,
  ) -> Result<PluginHandleExt<R>, PluginInvokeError> {
    if let Some(webview) = self.app().webviews().values().next() {
      let (tx, rx) = channel();
      let name = self.name();
      let config = self.raw_config().clone();
      let name = name.to_string();
      let config = serde_json::to_string(&config).unwrap();
      webview
        .with_webview(move |w| {
          unsafe {
            crate::macos::swift_register_plugin(
              &SRString::from(name.as_str()),
              init_fn(),
              &serde_json::to_string(&config).unwrap().as_str().into(),
              w.inner() as _,
            )
          };
          tx.send(()).unwrap();
        })
        .map_err(|_| PluginInvokeError::UnreachableWebview)?;
      rx.recv().unwrap();
    } else {
      unsafe {
        crate::macos::swift_register_plugin(
          &SRString::from(self.name()),
          init_fn(),
          &serde_json::to_string(&self.raw_config())
            .unwrap()
            .as_str()
            .into(),
          std::ptr::null(),
        )
      };
    }

    Ok(PluginHandleExt {
      name: self.name().to_string(),
      handle: self.app().clone(),
    })
  }
}

pub struct PluginHandleExt<R: Runtime> {
  name: String,
  handle: AppHandle<R>,
}

impl<R: Runtime> PluginHandleExt<R> {
  /// Executes the given Swift command.
  pub fn run_swift_plugin<T: DeserializeOwned>(
    &self,
    command: impl AsRef<str>,
    payload: impl Serialize,
  ) -> Result<T, PluginInvokeError> {
    let (tx, rx) = channel();

    run_command(
      &self.name,
      &self.handle,
      command,
      serde_json::to_value(payload).map_err(PluginInvokeError::CannotSerializePayload)?,
      move |response| {
        tx.send(response).unwrap();
      },
    )?;

    let response = rx.recv().unwrap();
    match response {
      Ok(r) => serde_json::from_value(r).map_err(PluginInvokeError::CannotDeserializeResponse),
      Err(r) => Err(
        serde_json::from_value::<ErrorResponse>(r)
          .map(Into::into)
          .map_err(PluginInvokeError::CannotDeserializeResponse)?,
      ),
    }
  }
}

pub(crate) fn run_command<R: Runtime, C: AsRef<str>, F: FnOnce(PluginResponse) + Send + 'static>(
  name: &str,
  _handle: &AppHandle<R>,
  command: C,
  payload: serde_json::Value,
  handler: F,
) -> Result<(), PluginInvokeError> {
  use std::{
    ffi::CStr,
    os::raw::{c_char, c_int, c_ulonglong},
  };

  let id: i32 = PENDING_PLUGIN_CALLS_ID.fetch_add(1, Ordering::Relaxed);
  PENDING_PLUGIN_CALLS
    .get_or_init(Default::default)
    .lock()
    .unwrap()
    .insert(id, Box::new(handler));

  unsafe {
    extern "C" fn plugin_command_response_handler(
      id: c_int,
      success: c_int,
      payload: *const c_char,
    ) {
      let payload = unsafe {
        assert!(!payload.is_null());
        CStr::from_ptr(payload)
      };

      if let Some(handler) = PENDING_PLUGIN_CALLS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .remove(&id)
      {
        let json = payload.to_str().unwrap();
        match serde_json::from_str(json) {
          Ok(payload) => {
            handler(if success == 1 {
              Ok(payload)
            } else {
              Err(payload)
            });
          }
          Err(err) => {
            handler(Err(format!("{err}, data: {json}").into()));
          }
        }
      }
    }

    extern "C" fn send_channel_data_handler(id: c_ulonglong, payload: *const c_char) {
      let payload = unsafe {
        assert!(!payload.is_null());
        CStr::from_ptr(payload)
      };

      if let Some(channel) = CHANNELS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .get(&(id as u32))
      {
        let payload: serde_json::Value = serde_json::from_str(payload.to_str().unwrap()).unwrap();
        let _ = channel.send(payload);
      }
    }

    crate::macos::swift_run_plugin_command(
      id,
      &name.into(),
      &command.as_ref().into(),
      &serde_json::to_string(&payload).unwrap().as_str().into(),
      crate::macos::PluginMessageCallback(plugin_command_response_handler),
      crate::macos::ChannelSendDataCallback(send_channel_data_handler),
    );
  }

  Ok(())
}
