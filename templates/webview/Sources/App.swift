import SwiftUI

// Native shell: shows Web/index.html in a WKWebView and exposes a few native features to JS.
// The app itself lives in Web/ and can be developed in any desktop browser.
@main
struct __APP_NAME__App: App {
    var body: some Scene {
        WindowGroup {
            WebAppView()
                .ignoresSafeArea(.container, edges: .bottom)
        }
    }
}
