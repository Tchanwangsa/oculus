// apple-speech: Apple's on-device recogniser (macOS 26 `SpeechAnalyzer`) as a
// command-line helper, so the Rust transcription pipeline can spawn it like
// ffmpeg. Built by `app/scripts/build-speech.mjs`; the Rust side is
// `app/src-tauri/src/transcribe/apple.rs`.
//
//   apple-speech locales
//     stdout {"available","reason","supported","installed","defaultLocale"}
//   apple-speech transcribe [--locale <id>] <audio>
//     stdout {"segments":[{"start","end","text","words":[{"start","end","text"}]}]}
//
// Exit 0 ok; 3 when this Mac cannot recognise speech on device (before
// macOS 26, or the recogniser is unavailable); 1 otherwise, with one line on
// stderr. Notes such as a model download also go to stderr.

import AVFoundation
import Foundation
import Speech

@main
struct AppleSpeech {
    // Synchronous on purpose: an async main needs the concurrency runtime at
    // launch, which Macs before 12 lack. Before 26 nothing async ever runs.
    static func main() {
        let args = Array(CommandLine.arguments.dropFirst())
        let command = args.first
        guard command == "locales" || command == "transcribe" else {
            fail("usage: apple-speech locales | apple-speech transcribe [--locale <id>] <audio>")
        }
        guard #available(macOS 26.0, *) else {
            if command == "locales" {
                emit(LocalesReport(available: false, reason: tooOld))
                exit(0)
            }
            unavailable(tooOld)
        }
        Task {
            if command == "locales" {
                await printLocales()
            } else {
                await transcribe(Array(args.dropFirst()))
            }
            exit(0)
        }
        dispatchMain()
    }
}

// MARK: - Output

func note(_ line: String) {
    FileHandle.standardError.write(Data((line + "\n").utf8))
}

/// One line on stderr, then exit 1.
func fail(_ message: String) -> Never {
    note(message.replacingOccurrences(of: "\n", with: " "))
    exit(1)
}

func unavailable(_ reason: String) -> Never {
    note(reason)
    exit(3)
}

func emit<T: Encodable>(_ value: T) {
    do {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(value)
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data("\n".utf8))
    } catch {
        fail("could not encode the output: \(error)")
    }
}

let tooOld = "On-device speech needs macOS 26 or later"
let notOnThisMac = "On-device speech recognition is not available on this Mac"

// MARK: - locales

struct LocalesReport: Encodable {
    var available: Bool
    var reason: String?
    var supported: [String] = []
    var installed: [String] = []
    var defaultLocale: String?

    // Spelled out so a missing reason or locale is `null`, never absent.
    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(available, forKey: .available)
        try c.encode(reason, forKey: .reason)
        try c.encode(supported, forKey: .supported)
        try c.encode(installed, forKey: .installed)
        try c.encode(defaultLocale, forKey: .defaultLocale)
    }

    enum CodingKeys: String, CodingKey {
        case available, reason, supported, installed, defaultLocale
    }
}

@available(macOS 26.0, *)
func printLocales() async {
    guard SpeechTranscriber.isAvailable else {
        emit(LocalesReport(available: false, reason: notOnThisMac))
        return
    }
    let supported = await SpeechTranscriber.supportedLocales.map(\.identifier).sorted()
    let installed = await SpeechTranscriber.installedLocales.map(\.identifier).sorted()
    let fallback = await defaultLocale()
    emit(LocalesReport(
        available: true,
        reason: nil,
        supported: supported,
        installed: installed,
        defaultLocale: fallback?.identifier
    ))
}

/// English in the Mac's region, else US English. Never the system language
/// itself: the Mac may be set to another language while lectures are English.
/// Exact matches only — `supportedLocale(equivalentTo:)` answers an
/// unsupported region (en_TH) with en_GB on one run and en_US on the next.
@available(macOS 26.0, *)
func defaultLocale() async -> Locale? {
    let supported = await SpeechTranscriber.supportedLocales
    if let region = Locale.current.region,
       let found = matching(Locale(languageCode: .english, languageRegion: region), in: supported) {
        return found
    }
    return matching(Locale(identifier: "en_US"), in: supported)
}

/// The supported locale with `wanted`'s language and region, if there is one.
@available(macOS 26.0, *)
func matching(_ wanted: Locale, in supported: [Locale]) -> Locale? {
    supported.first {
        $0.language.languageCode == wanted.language.languageCode && $0.region == wanted.region
    }
}

// MARK: - transcribe

struct Word: Encodable {
    var start: Double
    var end: Double
    var text: String
}

struct Segment: Encodable {
    var start: Double
    var end: Double
    var text: String
    var words: [Word]
}

struct Transcript: Encodable {
    var segments: [Segment]
}

@available(macOS 26.0, *)
func transcribe(_ args: [String]) async {
    var localeId: String?
    var path: String?
    var rest = args[...]
    while let arg = rest.popFirst() {
        if arg == "--locale" {
            guard let value = rest.popFirst() else { fail("--locale needs a value") }
            localeId = value
        } else if path == nil {
            path = arg
        } else {
            fail("unexpected argument \(arg)")
        }
    }
    guard let path else { fail("usage: apple-speech transcribe [--locale <id>] <audio>") }

    guard SpeechTranscriber.isAvailable else { unavailable(notOnThisMac) }

    let locale: Locale
    if let localeId {
        let wanted = Locale(identifier: localeId)
        let supported = await SpeechTranscriber.supportedLocales
        // `equivalentTo` can name a locale the list lacks (th_TH → th), whose
        // model download then fails; only a listed one is taken.
        let near = await SpeechTranscriber.supportedLocale(equivalentTo: wanted)
            .flatMap { matching($0, in: supported) }
        guard let found = matching(wanted, in: supported) ?? near else {
            fail("on-device speech does not support the locale \(localeId)")
        }
        locale = found
    } else {
        guard let found = await defaultLocale() else {
            fail("on-device speech supports no English locale on this Mac")
        }
        locale = found
    }

    guard FileManager.default.fileExists(atPath: path) else { fail("no such file: \(path)") }
    let file: AVAudioFile
    do {
        file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
    } catch {
        fail("could not read \(path): \(error.localizedDescription)")
    }

    do {
        let segments = try await recognise(file, locale: locale)
        emit(Transcript(segments: segments))
    } catch {
        fail("on-device speech failed: \(error.localizedDescription)")
    }
}

@available(macOS 26.0, *)
func recognise(_ file: AVAudioFile, locale: Locale) async throws -> [Segment] {
    let transcriber = SpeechTranscriber(
        locale: locale,
        transcriptionOptions: [],
        reportingOptions: [],
        attributeOptions: [.audioTimeRange]
    )
    // A request comes back even for an installed model; only a missing one is news.
    let installed = await AssetInventory.status(forModules: [transcriber]) == .installed
    if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
        if !installed { note("downloading the speech model for \(locale.identifier)") }
        try await request.downloadAndInstall()
        if !installed { note("speech model installed") }
    }

    // Results stream while the analyzer reads; collect them alongside.
    let collect = Task { () throws -> [Segment] in
        var segments: [Segment] = []
        for try await result in transcriber.results {
            var words: [Word] = []
            for run in result.text.runs {
                let text = String(result.text[run.range].characters)
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                if text.isEmpty { continue }
                if let range = run.audioTimeRange {
                    words.append(Word(start: range.start.seconds, end: range.end.seconds, text: text))
                } else if !words.isEmpty {
                    // Untimed text rides on the word before, so no words are lost.
                    let glue = text.first.map { $0.isPunctuation } == true ? "" : " "
                    words[words.count - 1].text += glue + text
                }
            }
            let text = String(result.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)
            if text.isEmpty { continue }
            segments.append(Segment(
                start: result.range.start.seconds,
                end: result.range.end.seconds,
                text: text,
                words: words
            ))
        }
        return segments
    }

    let analyzer = SpeechAnalyzer(modules: [transcriber])
    do {
        if let last = try await analyzer.analyzeSequence(from: file) {
            try await analyzer.finalizeAndFinish(through: last)
        } else {
            await analyzer.cancelAndFinishNow()
        }
    } catch {
        collect.cancel()
        throw error
    }
    return try await collect.value
}
