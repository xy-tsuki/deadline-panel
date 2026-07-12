import Foundation

enum NativeParsedCommand {
    case add(NewDeadlineInput)
    case complete(String)
    case delete(String)
    case update(String, NativeCommandUpdateFields)
}

struct NativeCommandUpdateFields {
    var title: String?
    var dueAt: String?
    var priority: String?
    var notes: String?
}

struct NativeCommandParseResult {
    var command: NativeParsedCommand?
    var error: String?

    var isValid: Bool {
        command != nil && error == nil
    }
}

enum NativeCommandParser {
    static func parse(_ input: String) -> NativeCommandParseResult {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("/") else {
            return .init(error: NativeStrings.current.commandNeedsSlash)
        }

        let parts = splitCommand(trimmed)
        let name = parts.name
        let rest = parts.rest
        let args = parseKeyValues(rest)

        switch name {
        case "/add":
            return parseAdd(args: args)
        case "/complete":
            return parseTargetCommand(rest: rest, args: args, kind: .complete)
        case "/delete":
            return parseTargetCommand(rest: rest, args: args, kind: .delete)
        case "/update":
            return parseUpdate(args: args)
        default:
            return .init(error: NativeStrings.current.supportedCommands)
        }
    }

    private static func parseAdd(args: [String: String]) -> NativeCommandParseResult {
        guard let title = args["title"], !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return .init(error: NativeStrings.current.missingTitle)
        }
        guard let due = args["due"] else {
            return .init(error: NativeStrings.current.missingDue)
        }
        guard let dueAt = normalizedDue(due) else {
            return .init(error: NativeStrings.current.dueUnrecognized)
        }
        guard let priority = normalizedPriority(args["priority"] ?? "medium") else {
            return .init(error: NativeStrings.current.priorityFormatError)
        }

        return .init(
            command: .add(
                NewDeadlineInput(
                    title: title,
                    dueAt: dueAt,
                    priority: priority,
                    notes: args["notes"] ?? "",
                    source: "command"
                )
            )
        )
    }

    private enum TargetCommandKind {
        case complete
        case delete
    }

    private static func parseTargetCommand(rest: String, args: [String: String], kind: TargetCommandKind) -> NativeCommandParseResult {
        let target = args["id"] ?? args["title"] ?? rest.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !target.isEmpty else {
            return .init(error: NativeStrings.current.commandNeedsTarget)
        }
        switch kind {
        case .complete:
            return .init(command: .complete(target))
        case .delete:
            return .init(command: .delete(target))
        }
    }

    private static func parseUpdate(args: [String: String]) -> NativeCommandParseResult {
        guard let target = args["id"] ?? args["title"], !target.isEmpty else {
            return .init(error: NativeStrings.current.commandNeedsTarget)
        }

        var fields = NativeCommandUpdateFields()
        fields.title = args["newTitle"]
        fields.notes = args["notes"]
        if let due = args["due"] {
            guard let dueAt = normalizedDue(due) else {
                return .init(error: NativeStrings.current.dueUnrecognized)
            }
            fields.dueAt = dueAt
        }
        if let rawPriority = args["priority"] {
            guard let priority = normalizedPriority(rawPriority) else {
                return .init(error: NativeStrings.current.priorityFormatError)
            }
            fields.priority = priority
        }

        return .init(command: .update(target, fields))
    }

    private static func splitCommand(_ input: String) -> (name: String, rest: String) {
        guard let firstSpace = input.firstIndex(where: { $0.isWhitespace }) else {
            return (input, "")
        }
        return (
            String(input[..<firstSpace]),
            String(input[input.index(after: firstSpace)...])
        )
    }

    private static func parseKeyValues(_ text: String) -> [String: String] {
        let pattern = #"(\w+)=(?:"([^"]*)"|“([^”]*)”|‘([^’]*)’|([^\s"“”‘’]+))"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return [:]
        }

        var args: [String: String] = [:]
        let nsRange = NSRange(text.startIndex..<text.endIndex, in: text)
        regex.enumerateMatches(in: text, range: nsRange) { match, _, _ in
            guard let match,
                  let keyRange = Range(match.range(at: 1), in: text)
            else {
                return
            }

            for index in 2..<match.numberOfRanges {
                let range = match.range(at: index)
                guard range.location != NSNotFound,
                      let valueRange = Range(range, in: text)
                else {
                    continue
                }
                args[String(text[keyRange])] = String(text[valueRange])
                return
            }
        }
        return args
    }

    private static func normalizedPriority(_ value: String) -> String? {
        let normalized = value.lowercased()
        return ["low", "medium", "high", "urgent"].contains(normalized) ? normalized : nil
    }

    private static func normalizedDue(_ value: String) -> String? {
        let normalized = value
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "/", with: "-")

        if let date = ISO8601DateFormatter.deadlinePanelCommandDate(from: normalized) {
            return ISO8601DateFormatter.deadlinePanelString(from: date)
        }

        return ISO8601DateFormatter.deadlinePanelDate(from: normalized)
            .map { ISO8601DateFormatter.deadlinePanelString(from: $0) }
    }
}
