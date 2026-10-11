#!/usr/bin/env bash
# Writes smoke.sh's inputs with a full ffmpeg (a distribution's, with its GPL
# encoders): one file per container and codec family a student's module
# videos and recordings arrive in. Only the fixtures need those encoders; the
# build under test decodes them.
#
# Usage: fixtures.sh <out-dir>
set -euo pipefail

mkdir -p "${1:?usage: fixtures.sh <out-dir>}"
cd "$1"

ff() { ffmpeg -nostdin -hide_banner -loglevel error -y "$@"; }
video() { echo "testsrc2=size=320x180:rate=25:duration=$1"; }
tone() { echo "sine=frequency=440:sample_rate=48000:duration=$1"; }
av() { ff -f lavfi -i "$(video "$1")" -f lavfi -i "$(tone "$1")" "${@:2}"; }

# Echo360's shape, long enough for the 14 s lead-in trim.
av 20 -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac -b:a 64k h264-aac.mp4
av 5 -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac -b:a 64k -f mov h264-aac.mov
av 5 -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -b:v 200k -c:a libopus -b:a 48k vp9-opus.webm
av 5 -c:v libx265 -preset ultrafast -x265-params log-level=error -c:a ac3 hevc-ac3.mkv
av 5 -c:v libaom-av1 -usage realtime -cpu-used 10 -b:v 100k -c:a libopus -b:a 48k av1-opus.mp4
av 5 -c:v mpeg2video -c:a mp2 -f mpegts mpeg2-mp2.ts
av 5 -c:v mpeg4 -c:a libmp3lame avi-mpeg4-mp3.avi
av 5 -c:v wmv2 -c:a wmav2 wmv2-wmav2.wmv
ff -f lavfi -i "$(video 5)" -c:v libx264 -preset veryfast -pix_fmt yuv420p no-audio.mp4
# A phone recording held upright: the display matrix makes ffmpeg insert
# a transpose filter.
ff -display_rotation 90 -i no-audio.mp4 -c copy rotated.mp4

ff -f lavfi -i "$(tone 5)" -c:a libmp3lame -b:a 64k speech.mp3
ff -f lavfi -i "$(tone 5)" -c:a aac -b:a 64k speech.m4a
ff -f lavfi -i "$(tone 5)" -c:a pcm_s16le speech.wav
ff -f lavfi -i "$(tone 5)" -c:a flac speech.flac
ff -f lavfi -i "$(tone 5)" -c:a libvorbis speech.ogg

ls -l
