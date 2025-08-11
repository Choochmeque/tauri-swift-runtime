![Crates.io Version](https://img.shields.io/crates/v/tauri-swift-runtime)

# Tauri Swift Runtime

A Swift runtime bridge for Tauri applications, enabling native Swift plugin development for macOS and iOS.

## Overview

This library provides the infrastructure to write and execute Swift plugins within Tauri applications. It handles the FFI (Foreign Function Interface) between Rust and Swift, allowing developers to access native Apple APIs through Swift while maintaining the cross-platform benefits of Tauri.

## Features

- **Swift Plugin System**: Write native plugins in Swift that can be called from Tauri
- **Type-Safe Communication**: Bridge between Rust and Swift with proper type conversions
- **Platform Support**: Works on macOS (10.13+) and iOS (11+)

## Installation

### Rust Side

Add to your `Cargo.toml`:

```toml
[dependencies]
tauri-swift-runtime = "0.1.0"
```

### Swift Side

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/yourusername/tauri-swift-runtime", from: "0.1.0")
]
```

## Usage

### Creating a Swift Plugin

Extend the `Plugin` class and implement your methods:

```swift
import TauriSwiftRuntime

class MyPlugin: Plugin {
    @objc func myMethod(_ invoke: Invoke) {
        // Your Swift code here
        invoke.success(["message": "Hello from Swift!"])
    }
    
    @objc func asyncMethod(_ invoke: Invoke, completionHandler: @escaping (NSError?) -> Void) {
        // Async operations
        DispatchQueue.main.async {
            invoke.success(["result": "Async complete"])
            completionHandler(nil)
        }
    }
}
```

### Rust Integration

Use the provided macros to bind your Swift plugin:

```rust
use tauri_swift_runtime::swift_plugin_binding;

swift_plugin_binding!(init_my_plugin);

// In your Tauri setup
app.setup(|app| {
    // Initialize your Swift plugin
    init_my_plugin();
    Ok(())
});
```

### JavaScript Usage

Call your Swift plugin from JavaScript:

```javascript
await invoke('plugin:myplugin|myMethod', { 
    arg1: 'value' 
});
```

## License

MIT License - see [LICENSE](LICENSE) file for details.

Copyright (c) 2025 Vladimir Pankratov
