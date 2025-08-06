mod desktop;
mod macos;

pub use desktop::{PluginApiExt, PluginHandleExt, PluginInvokeError};

#[doc(hidden)]
pub use swift_rs;

/// Setups the binding that initializes a Swift plugin.
#[macro_export]
macro_rules! swift_plugin_binding {
  ($fn_name: ident) => {
    $crate::swift_rs::swift!(fn $fn_name() -> *const ::std::ffi::c_void);
  }
}
