import SwiftUI
import UIKit
import WebKit

/// WKWebView showing the bundled web app.
struct WebAppView: UIViewRepresentable {
    func makeCoordinator() -> NativeBridge { NativeBridge() }

    func makeUIView(context: Context) -> WKWebView {
        let bridge = context.coordinator
        let config = WKWebViewConfiguration()
        // JS: window.webkit.messageHandlers.native.postMessage({ op, ...args }) -> Promise
        config.userContentController.addScriptMessageHandler(bridge, contentWorld: .page, name: "native")
        config.allowsInlineMediaPlayback = true

        let web = WKWebView(frame: .zero, configuration: config)
        web.isOpaque = false
        web.backgroundColor = .systemBackground
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.allowsLinkPreview = false
        // Lets Safari on a Mac inspect the page, if one is ever around.
        if #available(iOS 16.4, *) { web.isInspectable = true }
        web.uiDelegate = bridge
        bridge.webView = web

        if let index = Bundle.module.url(forResource: "index", withExtension: "html", subdirectory: "Web") {
            web.loadFileURL(index, allowingReadAccessTo: index.deletingLastPathComponent())
        } else {
            web.loadHTMLString("<h1 style='font-family:-apple-system'>Web/index.html is missing</h1>", baseURL: nil)
        }
        return web
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

/// JS -> Swift calls. Every message `{ op, ... }` gets a reply (resolves the JS promise).
/// Add your own native features here and mirror them in Web/native.js.
final class NativeBridge: NSObject, WKScriptMessageHandlerWithReply, WKUIDelegate {
    weak var webView: WKWebView?
    private let defaults = UserDefaults.standard

    func userContentController(_ userContentController: WKUserContentController,
                               didReceive message: WKScriptMessage,
                               replyHandler: @escaping (Any?, String?) -> Void) {
        guard let body = message.body as? [String: Any], let op = body["op"] as? String else {
            replyHandler(nil, "invalid message"); return
        }
        switch op {
        case "device":
            replyHandler([
                "name": UIDevice.current.name,
                "model": UIDevice.current.model,
                "system": UIDevice.current.systemName + " " + UIDevice.current.systemVersion,
                "native": true,
            ], nil)
        case "haptic":
            UIImpactFeedbackGenerator(style: .medium).impactOccurred()
            replyHandler(true, nil)
        case "storeGet":
            replyHandler(defaults.string(forKey: "web." + (body["key"] as? String ?? "")), nil)
        case "storeSet":
            defaults.set(body["value"] as? String, forKey: "web." + (body["key"] as? String ?? ""))
            replyHandler(true, nil)
        case "share":
            share(text: body["text"] as? String ?? "")
            replyHandler(true, nil)
        default:
            replyHandler(nil, "unknown op: \(op)")
        }
    }

    private func share(text: String) {
        guard let web = webView, let root = web.window?.rootViewController else { return }
        let vc = UIActivityViewController(activityItems: [text], applicationActivities: nil)
        if let pop = vc.popoverPresentationController {
            pop.sourceView = web
            pop.sourceRect = CGRect(x: web.bounds.midX, y: web.bounds.midY, width: 1, height: 1)
        }
        topmost(root).present(vc, animated: true)
    }

    private func topmost(_ vc: UIViewController) -> UIViewController {
        var v = vc
        while let p = v.presentedViewController { v = p }
        return v
    }

    // alert() / confirm() from JS: WKWebView ignores them unless the app shows them.

    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String,
                 initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        guard let root = webView.window?.rootViewController else { completionHandler(); return }
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler() })
        topmost(root).present(a, animated: true)
    }

    func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String,
                 initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        guard let root = webView.window?.rootViewController else { completionHandler(false); return }
        let a = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        a.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in completionHandler(false) })
        a.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler(true) })
        topmost(root).present(a, animated: true)
    }
}
