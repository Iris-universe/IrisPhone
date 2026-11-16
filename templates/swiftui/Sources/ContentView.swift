import Observation
import SwiftUI

@Observable
final class CounterModel {
    var taps = 0
}

struct ContentView: View {
    @Environment(CounterModel.self) private var model

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Image(systemName: "iphone.gen3")
                    .font(.system(size: 64))
                    .foregroundStyle(.tint)
                Text("__DISPLAY_NAME__")
                    .font(.largeTitle.bold())
                Text("Built on a PC, no Mac involved.")
                    .foregroundStyle(.secondary)
                Button("Tapped \(model.taps) times") {
                    model.taps += 1
                }
                .buttonStyle(.borderedProminent)
            }
            .padding()
        }
    }
}
