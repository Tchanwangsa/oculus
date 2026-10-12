#!/usr/bin/env bash
# Runs every ffmpeg command shape the app uses against fixtures.sh's files and
# checks exit codes and outputs, then records the binary's -version,
# -buildconf and -L in the report directory. Runs on Windows and macOS: Studio's
# H.264 encoder is the OS's own (h264_mf, h264_videotoolbox), so that shape
# falls back to stream-copied segments on a runner whose OS cannot encode. The shapes mirror the call sites
# in app/src-tauri/src (sources/echo360/, transcribe/, the frame readers,
# studio/render/).
#
# Usage: smoke.sh <ffmpeg> <fixtures-dir> <report-dir>
set -uo pipefail

usage="usage: smoke.sh <ffmpeg> <fixtures-dir> <report-dir>"
ff=$(cd "$(dirname "${1:?$usage}")" && pwd)/$(basename "$1")
fixtures=$(cd "${2:?$usage}" && pwd)
mkdir -p "${3:?$usage}"
report=$(cd "$3" && pwd)
scratch=$(mktemp -d)
cd "$scratch"

FRAME=$((160 * 90))
failures=0
pass() { echo "ok   $*"; }
fail() { echo "FAIL $*"; failures=$((failures + 1)); }
size() { if [ -f "$1" ]; then wc -c < "$1" | tr -d ' '; else echo 0; fi; }

"$ff" -version > "$report/version.txt" 2>&1 && pass "-version" || fail "-version exited $?"
"$ff" -hide_banner -buildconf > "$report/buildconf.txt" 2>&1 || fail "-buildconf exited $?"
"$ff" -hide_banner -L > "$report/license.txt" 2>&1 || fail "-L exited $?"
if grep -Eq -- '--enable-(gpl|nonfree)' "$report/version.txt" "$report/buildconf.txt"; then
  fail "configuration enables GPL or nonfree code"
else
  pass "configuration has no --enable-gpl or --enable-nonfree"
fi
if grep -q "GNU Lesser General Public License" "$report/license.txt" && grep -q "either version 3" "$report/license.txt"; then
  pass "-L reports LGPL version 3 or later"
else
  fail "-L does not report LGPL version 3: $(head -3 "$report/license.txt")"
fi

# The components the app's command shapes need, and no x264.
has() { "$ff" -hide_banner "-$1" 2>&1 | awk -v n="$2" '{ k = split($2, a, ","); for (i = 1; i <= k; i++) if (a[i] == n) found = 1; if ($1 == n) found = 1 } END { exit !found }'; }
missing=
for c in encoders:aac encoders:libopus encoders:mjpeg encoders:pcm_s16le \
  demuxers:rawvideo demuxers:concat demuxers:mov demuxers:ogg demuxers:wav \
  muxers:mp4 muxers:ogg muxers:wav muxers:image2 muxers:rawvideo \
  filters:scale filters:fps filters:format bsfs:aac_adtstoasc; do
  has "${c%%:*}" "${c#*:}" || missing="$missing ${c#*:}"
done
[ -z "$missing" ] && pass "every component the app needs is built in" || fail "built without:$missing"
if has encoders libx264; then fail "libx264 is built in"; else pass "libx264 is not built in"; fi
os_encoder=
for e in h264_videotoolbox h264_mf; do has encoders "$e" && os_encoder=$e; done
[ -n "$os_encoder" ] && pass "OS H.264 encoder built in: $os_encoder" || fail "no OS H.264 encoder (h264_videotoolbox, h264_mf)"

# Echo360's lead-in trim: a stream copy into mp4.
if "$ff" -y -ss 14 -i "$fixtures/h264-aac.mp4" -c copy source.mp4 2> trim.err; then
  bytes=$("$ff" -v error -nostdin -ss 1 -i source.mp4 -frames:v 1 -vf scale=160:90,format=gray -f rawvideo - | wc -c)
  [ "$bytes" -eq "$FRAME" ] && pass "trim: source.mp4 decodes" || fail "trim: frame read $bytes bytes"
else
  fail "trim exited nonzero: $(tail -3 trim.err)"
fi

# Transcription: extract speech-rate Opus, decode it to PCM, cut parts.
for input in h264-aac.mp4 h264-aac.mov vp9-opus.webm hevc-ac3.mkv av1-opus.mp4 mpeg2-mp2.ts \
  avi-mpeg4-mp3.avi wmv2-wmav2.wmv speech.mp3 speech.m4a speech.wav speech.flac speech.ogg; do
  rm -f audio.ogg audio.wav part.ogg tail.ogg
  if ! "$ff" -nostdin -hide_banner -nostats -y -i "$fixtures/$input" -map 0:a:0 -vn -ac 1 -ar 16000 \
    -c:a libopus -b:a 24k -vbr constrained -application voip audio.ogg 2> extract.err; then
    fail "$input: extract exited nonzero: $(tail -3 extract.err)"; continue
  fi
  grep -q "Duration: " extract.err || fail "$input: no Duration: in stderr"
  [ "$(head -c 4 audio.ogg)" = OggS ] || fail "$input: audio.ogg is not Ogg"
  "$ff" -i audio.ogg -ac 1 -ar 16000 -c:a pcm_s16le audio.wav 2> wav.err || fail "$input: ogg to wav: $(tail -3 wav.err)"
  # At least 4 s of 16 kHz mono 16-bit PCM from each 5 s or longer fixture.
  wav=$(size audio.wav)
  [ "$wav" -ge $((16000 * 2 * 4)) ] || fail "$input: audio.wav is $wav bytes"
  "$ff" -y -ss 1 -i audio.ogg -t 2 -c copy part.ogg 2> part.err && [ "$(size part.ogg)" -gt 0 ] ||
    fail "$input: part copy: $(tail -3 part.err)"
  "$ff" -y -ss 2 -i audio.ogg -c copy tail.ogg 2> tail.err && [ "$(size tail.ogg)" -gt 0 ] ||
    fail "$input: tail copy: $(tail -3 tail.err)"
  pass "$input: extract, wav ($wav bytes), part copies"
done

if "$ff" -nostdin -hide_banner -nostats -y -i "$fixtures/no-audio.mp4" -map 0:a:0 -vn -ac 1 -ar 16000 \
  -c:a libopus -b:a 24k -vbr constrained -application voip audio.ogg 2> none.err; then
  fail "no-audio.mp4: extract succeeded"
elif grep -q "matches no streams" none.err; then
  pass "no-audio.mp4: extract fails with 'matches no streams'"
else
  fail "no-audio.mp4: unexpected error: $(tail -3 none.err)"
fi

# Thumbnails and slide-change frames.
for input in h264-aac.mp4 h264-aac.mov vp9-opus.webm hevc-ac3.mkv av1-opus.mp4 mpeg2-mp2.ts \
  avi-mpeg4-mp3.avi wmv2-wmav2.wmv no-audio.mp4 rotated.mp4; do
  rm -f out.jpg
  "$ff" -v error -nostdin -y -ss 2 -i "$fixtures/$input" -frames:v 1 -vf 'scale=min(768\,iw):-2' -q:v 3 out.jpg 2> jpg.err
  if [ "$(size out.jpg)" -gt 0 ] && [ "$(head -c 2 out.jpg | od -An -tx1 | tr -d ' \n')" = ffd8 ]; then :; else
    fail "$input: thumbnail: $(tail -3 jpg.err)"; continue
  fi
  one=$("$ff" -v error -nostdin -ss 2 -i "$fixtures/$input" -frames:v 1 -vf scale=160:90,format=gray -f rawvideo - 2> one.err | wc -c)
  [ "$one" -eq "$FRAME" ] || { fail "$input: single frame $one bytes: $(tail -3 one.err)"; continue; }
  all=$("$ff" -v info -nostdin -hide_banner -nostats -i "$fixtures/$input" -vf fps=1,scale=160:90,format=gray -f rawvideo - 2> all.err | wc -c)
  grep -q "Duration: " all.err || fail "$input: no Duration: in frame scan stderr"
  [ $((all % FRAME)) -eq 0 ] && [ $((all / FRAME)) -ge 4 ] || { fail "$input: fps=1 scan $all bytes: $(tail -3 all.err)"; continue; }
  end=$("$ff" -v info -nostdin -hide_banner -nostats -sseof -3 -i "$fixtures/$input" -vf fps=1,scale=160:90,format=gray -f rawvideo - 2> end.err | wc -c)
  [ $((end % FRAME)) -eq 0 ] && [ $((end / FRAME)) -ge 2 ] && [ $((end / FRAME)) -le 4 ] ||
    { fail "$input: -sseof scan $end bytes: $(tail -3 end.err)"; continue; }
  pass "$input: thumbnail $(size out.jpg) bytes, frame, $((all / FRAME)) scan frames, $((end / FRAME)) tail frames"
done

# Studio: raw RGBA frames on stdin encoded by the OS encoder as two segments,
# joined by the concat demuxer beside AAC narration, and the narration alone as
# m4a. The encoder args are quality.rs's. A runner without the OS encoder (a
# VM without a media engine) cannot run it, so the segments are then stream
# copies of the H.264 fixture: the concat and AAC mux are still checked.
case "$os_encoder" in
  h264_videotoolbox) encoder_args=(-q:v 65) ;;
  *) encoder_args=(-rate_control u_vbr -b:v 2000k) ;;
esac
encode_segment() {
  "$ff" -v error -nostdin -ss "$1" -i "$fixtures/h264-aac.mp4" -frames:v 30 -vf scale=320:180,format=rgba \
    -f rawvideo - 2> raw.err |
    "$ff" -hide_banner -v error -y -f rawvideo -pix_fmt rgba -s 320x180 -r 30 -i - -an \
      -vf scale=out_color_matrix=bt709:out_range=tv -pix_fmt yuv420p -c:v "$os_encoder" "${encoder_args[@]}" \
      -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv -f mp4 "seg$1.mp4" 2> seg.err
}
studio_ok=1
encoded=1
if [ -n "$os_encoder" ] && ! encode_segment 1; then
  echo "::warning::$os_encoder cannot encode on this runner; Studio's segments are stream copies here"
  encoded=0
fi
[ -n "$os_encoder" ] || encoded=0
for seg in 1 2; do
  if [ "$encoded" = 1 ]; then
    [ "$seg" = 1 ] || encode_segment "$seg" ||
      { fail "studio: segment $seg: $(tail -3 seg.err) $(tail -3 raw.err)"; studio_ok=0; }
  else
    "$ff" -v error -nostdin -y -ss "$seg" -t 1 -i "$fixtures/h264-aac.mp4" -an -c:v copy "seg$seg.mp4" 2> seg.err ||
      { fail "studio: segment $seg copy: $(tail -3 seg.err)"; studio_ok=0; }
  fi
done
if [ "$studio_ok" = 1 ]; then
  printf "file '%s'\nfile '%s'\n" "$PWD/seg1.mp4" "$PWD/seg2.mp4" > concat.txt
  if "$ff" -hide_banner -v error -y -f concat -safe 0 -i concat.txt -i "$fixtures/speech.wav" -map 0:v -c:v copy \
    -map 1:a -c:a aac -b:a 160k -movflags +faststart -f mp4 joined.mp4 2> join.err; then
    frames=$("$ff" -v error -nostdin -i joined.mp4 -vf scale=160:90,format=gray -f rawvideo - 2> joined.err | wc -c)
    if [ "$encoded" = 1 ]; then
      [ "$frames" -eq $((60 * FRAME)) ] && pass "studio: $os_encoder segments joined with AAC, 60 frames" ||
        fail "studio: joined.mp4 decodes to $((frames / FRAME)) frames: $(tail -3 joined.err)"
    else
      [ "$frames" -gt 0 ] && [ $((frames % FRAME)) -eq 0 ] && pass "studio: stream-copied segments joined with AAC, $((frames / FRAME)) frames" ||
        fail "studio: joined.mp4 decodes to $frames bytes: $(tail -3 joined.err)"
    fi
  else
    fail "studio: concat and AAC mux: $(tail -3 join.err)"
  fi
fi
"$ff" -hide_banner -v error -y -i "$fixtures/speech.wav" -map 0:a -c:a aac -b:a 160k -movflags +faststart \
  -f mp4 narration.m4a 2> m4a.err && [ "$(size narration.m4a)" -gt 0 ] && pass "studio: narration as m4a" ||
  fail "studio: narration as m4a: $(tail -3 m4a.err)"

echo "binary: $(size "$ff") bytes"
cd / && rm -rf "$scratch"
if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
