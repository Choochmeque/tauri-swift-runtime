// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#if os(iOS)
import UIKit

public class UIUtils {
    public static func centerPopover(rootViewController: UIViewController?, popoverController: UIViewController) {
        if let viewController = rootViewController {
            popoverController.popoverPresentationController?.sourceRect = CGRect(x: viewController.view.center.x, y: viewController.view.center.y, width: 0, height: 0)
            popoverController.popoverPresentationController?.sourceView = viewController.view
            popoverController.popoverPresentationController?.permittedArrowDirections = UIPopoverArrowDirection.up
        }
    }
}
#elseif os(macOS)
import AppKit

public class UIUtils {
    public static func centerPopover(rootViewController: NSViewController?, popoverController: NSViewController) {
        // macOS popover handling would go here if needed
        // This is a placeholder implementation
    }
}
#endif
