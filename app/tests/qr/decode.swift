// Decodes each PNG named on the command line with CoreImage's QR detector and
// prints a JSON array of the messages (null where none was found). Used only
// by qrDecode.test.ts, on macOS.
import CoreImage
import Foundation

let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])!
var messages: [Any] = []
for path in CommandLine.arguments.dropFirst() {
    let image = CIImage(contentsOf: URL(fileURLWithPath: path))
    let found = image.flatMap { detector.features(in: $0).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first }
    messages.append(found ?? NSNull())
}
let json = try JSONSerialization.data(withJSONObject: messages)
print(String(data: json, encoding: .utf8)!)
