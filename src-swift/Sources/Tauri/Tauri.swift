// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import Foundation
import SwiftRs
import WebKit
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

class PluginHandle {
  var instance: Plugin
  var loaded = false

  init(plugin: Plugin) {
    instance = plugin
  }
}

public class PluginManager {
  static let shared: PluginManager = PluginManager()
  #if os(iOS)
  public var viewController: UIViewController?
  #elseif os(macOS)
  public var viewController: NSViewController?
  #endif
  var plugins: [String: PluginHandle] = [:]
  var ipcDispatchQueue = DispatchQueue(label: "ipc")
  public var isSimEnvironment: Bool {
    #if targetEnvironment(simulator)
      return true
    #else
      return false
    #endif
  }

  public func assetUrl(fromLocalURL url: URL?) -> URL? {
    guard let inputURL = url else {
      return nil
    }

    return URL(string: "asset://localhost")!.appendingPathComponent(inputURL.path)
  }

  func onWebviewCreated(_ webview: WKWebView) {
    for (_, handle) in plugins {
      if !handle.loaded {
        handle.instance.load(webview: webview)
      }
    }
  }

  func load<P: Plugin>(name: String, plugin: P, config: String, webview: WKWebView?) {
    plugin.setConfig(config)
    let handle = PluginHandle(plugin: plugin)
    if let webview = webview {
      handle.instance.load(webview: webview)
      handle.loaded = true
    }
    plugins[name] = handle
  }

  func decodeTypeEncoding(_ encoding: String) -> String {
    switch encoding {
      case "v": return "Void"
      case "@": return "Object"
      case ":": return "Selector"
      case "i": return "Int32"
      case "q": return "Int64"
      case "d": return "Double"
      case "f": return "Float"
      case "B": return "Bool"
      case "@?": return "Block"
      default: return encoding // fallback to raw type code
    }
  }

  func invoke(name: String, invoke: Invoke) {
    if let plugin = plugins[name] {
      ipcDispatchQueue.async {
        let selectorWithCompletionHandler = Selector(("\(invoke.command):completionHandler:"))
        let selectorWithThrows = Selector(("\(invoke.command):error:"))

        if plugin.instance.responds(to: selectorWithCompletionHandler) {
          let completion: @convention(block) (NSError?) -> Void = { error in
            if let error = error {
              invoke.reject("Swift async error: \(error)")
            }
          }

          let blockObj: AnyObject = unsafeBitCast(completion, to: AnyObject.self)
          let imp = plugin.instance.method(for: selectorWithCompletionHandler)

          typealias Fn = @convention(c) (AnyObject, Selector, Invoke, AnyObject) -> Void
          let fn = unsafeBitCast(imp, to: Fn.self)
          fn(plugin.instance, selectorWithCompletionHandler, invoke, blockObj)
        } else if plugin.instance.responds(to: selectorWithThrows) {
          var error: NSError? = nil
          withUnsafeMutablePointer(to: &error) {
            let methodIMP: IMP! = plugin.instance.method(for: selectorWithThrows)
            unsafeBitCast(
              methodIMP, to: (@convention(c) (Any?, Selector, Invoke, OpaquePointer) -> Void).self)(
                plugin.instance, selectorWithThrows, invoke, OpaquePointer($0))
          }
          if let error = error {
            invoke.reject("\(error)")
            // TODO: app crashes without this leak
            let _ = Unmanaged.passRetained(error)
          }
        } else {
          let selector = Selector(("\(invoke.command):"))
          if plugin.instance.responds(to: selector) {
            plugin.instance.perform(selector, with: invoke)
          } else {
            // Print selectors for debugging
            let cls: AnyClass = object_getClass(plugin.instance)!
            var methodCount: UInt32 = 0
            var selectorInfo: [String] = []

            if let methodList = class_copyMethodList(cls, &methodCount) {
              for i in 0..<Int(methodCount) {
                let method = methodList[i]
                let sel = method_getName(method)
                let selName = NSStringFromSelector(sel)

                let argCount = method_getNumberOfArguments(method)
                var argTypes: [String] = []

                for j in 0..<argCount {
                  if let encoding = method_copyArgumentType(method, UInt32(j)) {
                    let typeStr = String(cString: encoding)
                    let readable = self.decodeTypeEncoding(typeStr)
                    argTypes.append(readable)
                    free(UnsafeMutableRawPointer(mutating: encoding))
                  }
                }

                let returnEncoding = method_copyReturnType(method)
                let returnStr = String(cString: returnEncoding)
                let readableReturn = self.decodeTypeEncoding(returnStr)
                free(returnEncoding)

                let userArgs = argTypes.dropFirst(2).joined(separator: ", ") // drop self and _cmd
                let signature = "\(selName): (\(userArgs)) -> \(readableReturn)"
                selectorInfo.append(signature)
              }
              free(methodList)
            }

            let available = selectorInfo.joined(separator: "\n")
            invoke.reject("No command \(invoke.command) found for plugin \(name).\nAvailable selectors:\n\(available)")
          }
        }
      }
    } else {
      invoke.reject("Plugin \(name) not initialized")
    }
  }
}

extension PluginManager: NSCopying {
  public func copy(with zone: NSZone? = nil) -> Any {
    return self
  }
}

@_cdecl("swift_register_plugin")
func registerPlugin(name: SRString, plugin: NSObject, config: SRString, webview: WKWebView?) {
  PluginManager.shared.load(
    name: name.toString(),
    plugin: plugin as! Plugin,
    config: config.toString(),
    webview: webview
  )
}

@_cdecl("on_webview_created")
func onWebviewCreated(webview: WKWebView, viewController: NSObject) {
  #if os(iOS)
  if let vc = viewController as? UIViewController {
    PluginManager.shared.viewController = vc
  }
  #elseif os(macOS)
  if let vc = viewController as? NSViewController {
    PluginManager.shared.viewController = vc
  }
  #endif
  PluginManager.shared.onWebviewCreated(webview)
}

@_cdecl("swift_run_plugin_command")
func runCommand(
  id: Int,
  name: SRString,
  command: SRString,
  data: SRString,
  callback: @escaping @convention(c) (Int, Bool, UnsafePointer<CChar>) -> Void,
  sendChannelData: @escaping @convention(c) (UInt64, UnsafePointer<CChar>) -> Void
) {
  let callbackId: UInt64 = 0
  let errorId: UInt64 = 1
  let invoke = Invoke(
    command: command.toString(), callback: callbackId, error: errorId,
    sendResponse: { (fn: UInt64, payload: String?) -> Void in
      let success = fn == callbackId
      callback(id, success, payload ?? "null")
    },
    sendChannelData: { (id: UInt64, payload: String) -> Void in
      sendChannelData(id, payload)
    }, data: data.toString())
  PluginManager.shared.invoke(name: name.toString(), invoke: invoke)
}
