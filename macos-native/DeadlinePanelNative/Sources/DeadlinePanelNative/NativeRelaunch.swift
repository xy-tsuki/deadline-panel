import AppKit

enum NativeRelaunch {
    @MainActor
    static func restart() {
        let appURL = Bundle.main.bundleURL
        guard appURL.pathExtension == "app" else {
            NSApp.terminate(nil)
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = [
            "-c",
            "sleep 0.25; /usr/bin/open -n \(shellQuoted(appURL.path))"
        ]
        try? process.run()
        NSApp.terminate(nil)
    }

    private static func shellQuoted(_ value: String) -> String {
        "'\(value.replacingOccurrences(of: "'", with: "'\\''"))'"
    }
}
