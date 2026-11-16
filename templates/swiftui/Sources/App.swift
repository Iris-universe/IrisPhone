import SwiftUI

@main
struct __APP_NAME__App: App {
    @State private var model = CounterModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(model)
        }
    }
}
