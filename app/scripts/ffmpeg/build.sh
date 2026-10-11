#!/usr/bin/env bash
# Builds Oculus's LGPL v3 ffmpeg from pinned upstream release tarballs:
# libopus and dav1d as static libraries, then FFmpeg configured with
# configure-flags.txt. Every tarball must match its pinned SHA-256, and
# FFmpeg's must carry a good signature from its release key.
#
# Usage: build.sh <target> <out-dir>     targets: windows-x64
#
# <out-dir> receives everything a release needs: the binary, the
# byte-identical source tarballs, these build files, the full configure line,
# config.log, the toolchain versions and SHA256SUMS. FFMPEG_WORK_DIR (default
# ./ffmpeg-work) holds downloads and build trees. docs/development.md covers
# the workflow that runs this.
set -euo pipefail

FFMPEG_VERSION=8.1.3
FFMPEG_SHA256=7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3
FFMPEG_KEY_FINGERPRINT=FCF986EA15E6E293A5644F10B4322F04D67658D8
OPUS_VERSION=1.6.1
OPUS_SHA256=6ffcb593207be92584df15b32466ed64bbec99109f007c82205f0194572411a1
DAV1D_VERSION=1.5.4
DAV1D_SHA256=686616b7c69eb88d44459391ab25cac13b6647a3b288835c5784e71c1514a5c5
# Raise when the build changes without an upstream version change; it names
# the release tag, ffmpeg-<version>-oculus<build>.
OCULUS_BUILD=1

usage="usage: build.sh <target> <out-dir> (targets: windows-x64)"
here=$(cd "$(dirname "$0")" && pwd)
target=${1:?$usage}
out_arg=${2:?$usage}
work=${FFMPEG_WORK_DIR:-$PWD/ffmpeg-work}
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)

die() { echo "build.sh: $*" >&2; exit 1; }

case "$target" in
  windows-x64)
    host=x86_64-w64-mingw32
    exe_ext=.exe
    meson_system=windows
    meson_cpu=x86_64
    # Win32 threads and a fully static link: the binary imports Windows
    # system DLLs only, never the MinGW runtime's.
    target_flags=(--enable-cross-compile --target-os=mingw32 --arch=x86_64
      "--cross-prefix=$host-" --enable-w32threads)
    target_ldflags="-static -Wl,--no-insert-timestamp"
    ;;
  *) die "unknown target '$target'; $usage" ;;
esac

if command -v sha256sum >/dev/null; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$out_arg" "$work/downloads"
out=$(cd "$out_arg" && pwd)
prefix="$work/$target/prefix"
build="$work/$target/build"
rm -rf "$work/$target"
mkdir -p "$prefix" "$build"

fetch() {
  local url=$1 sum=$2 file="$work/downloads/${1##*/}"
  if [ ! -f "$file" ]; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  local got
  got=$(sha256 "$file")
  [ "$got" = "$sum" ] || die "${file##*/} has SHA-256 $got, expected $sum"
  echo "verified ${file##*/} $got"
}

ffmpeg_tar=ffmpeg-$FFMPEG_VERSION.tar.xz
opus_tar=opus-$OPUS_VERSION.tar.gz
dav1d_tar=dav1d-$DAV1D_VERSION.tar.xz
fetch "https://ffmpeg.org/releases/$ffmpeg_tar" "$FFMPEG_SHA256"
fetch "https://downloads.xiph.org/releases/opus/$opus_tar" "$OPUS_SHA256"
fetch "https://downloads.videolan.org/pub/videolan/dav1d/$DAV1D_VERSION/$dav1d_tar" "$DAV1D_SHA256"

# The signature is checked against the committed key, whose fingerprint is
# pinned above. FFMPEG_SKIP_GPG=1 skips it on a machine without gpg; the
# SHA-256 pin still applies. CI always checks it.
if [ "${FFMPEG_SKIP_GPG:-}" = 1 ]; then
  echo "skipping the FFmpeg signature check (FFMPEG_SKIP_GPG=1)"
else
  [ -f "$work/downloads/$ffmpeg_tar.asc" ] ||
    curl -fsSL --retry 3 -o "$work/downloads/$ffmpeg_tar.asc" "https://ffmpeg.org/releases/$ffmpeg_tar.asc"
  gnupg=$(mktemp -d)
  gpg --homedir "$gnupg" --batch --quiet --import "$here/ffmpeg-release-key.asc"
  fingerprint=$(gpg --homedir "$gnupg" --batch --with-colons --fingerprint | awk -F: '/^fpr:/ { print $10; exit }')
  [ "$fingerprint" = "$FFMPEG_KEY_FINGERPRINT" ] || die "release key fingerprint $fingerprint, expected $FFMPEG_KEY_FINGERPRINT"
  gpg --homedir "$gnupg" --batch --status-fd 1 --verify "$work/downloads/$ffmpeg_tar.asc" "$work/downloads/$ffmpeg_tar" 2>/dev/null |
    grep -q "^\[GNUPG:\] VALIDSIG .* $FFMPEG_KEY_FINGERPRINT\$" || die "$ffmpeg_tar has no good signature from $FFMPEG_KEY_FINGERPRINT"
  rm -rf "$gnupg"
  echo "verified $ffmpeg_tar signature by $FFMPEG_KEY_FINGERPRINT"
fi

for tar in "$ffmpeg_tar" "$opus_tar" "$dav1d_tar"; do tar -xf "$work/downloads/$tar" -C "$build"; done

echo "building libopus $OPUS_VERSION"
mkdir -p "$build/opus"
(cd "$build/opus" && "../opus-$OPUS_VERSION/configure" ${host:+--host=$host} --prefix="$prefix" \
  --disable-shared --enable-static --disable-doc --disable-extra-programs && make -j"$jobs" && make install) \
  > "$build/opus.log" 2>&1 || { tail -50 "$build/opus.log"; die "libopus failed; see $build/opus.log"; }

echo "building dav1d $DAV1D_VERSION"
meson_cross=()
if [ -n "${host:-}" ]; then
  cat > "$build/cross.meson" <<EOF
[binaries]
c = '$host-gcc'
ar = '$host-ar'
strip = '$host-strip'
windres = '$host-windres'

[host_machine]
system = '$meson_system'
cpu_family = '$meson_cpu'
cpu = '$meson_cpu'
endian = 'little'
EOF
  meson_cross=(--cross-file "$build/cross.meson")
fi
(meson setup "${meson_cross[@]}" "$build/dav1d" "$build/dav1d-$DAV1D_VERSION" --prefix="$prefix" --libdir=lib \
  --buildtype=release --default-library=static -Denable_tools=false -Denable_tests=false \
  -Denable_examples=false -Denable_docs=false && ninja -C "$build/dav1d" install) \
  > "$build/dav1d.log" 2>&1 || { tail -50 "$build/dav1d.log"; die "dav1d failed; see $build/dav1d.log"; }

echo "configuring FFmpeg $FFMPEG_VERSION"
flags=()
while IFS= read -r line; do
  line=${line%%#*}
  line=${line%"${line##*[![:space:]]}"}
  [ -n "$line" ] && flags+=("$line")
done < "$here/configure-flags.txt"
configure=("../ffmpeg-$FFMPEG_VERSION/configure" --prefix="$prefix" "${flags[@]}" "${target_flags[@]}"
  --pkg-config=pkg-config --pkg-config-flags=--static
  "--extra-cflags=-I$prefix/include" "--extra-ldflags=-L$prefix/lib $target_ldflags")
mkdir -p "$build/ffmpeg"
cd "$build/ffmpeg"
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" PKG_CONFIG_PATH=
"${configure[@]}" > configure.out 2>&1 || {
  cp ffbuild/config.log "$out/config-$target.log" 2>/dev/null || true
  tail -40 ffbuild/config.log 2>/dev/null; cat configure.out; die "FFmpeg configure failed"; }
grep -q '^#define FFMPEG_LICENSE "LGPL version 3 or later"$' config.h || die "configured licence is not LGPL version 3 or later"
grep -q '^#define CONFIG_GPL 0$' config.h && grep -q '^#define CONFIG_NONFREE 0$' config.h || die "GPL or nonfree code is enabled"

echo "building FFmpeg $FFMPEG_VERSION"
make -j"$jobs" > make.log 2>&1 || { tail -60 make.log; die "FFmpeg build failed"; }

binary="ffmpeg-$target$exe_ext"
cp "ffmpeg$exe_ext" "$out/$binary"
cp ffbuild/config.log "$out/config-$target.log"
{
  printf '%q ' "${configure[@]}"
  echo
} > "$out/configure-$target.txt"

{
  echo "target: $target"
  echo "built: $(uname -srm)"
  if [ -n "${host:-}" ]; then
    "$host-gcc" --version | head -1
    "$host-ld" --version | head -1
  else
    cc --version | head -1
  fi
  nasm -v
  echo "meson $(meson --version)"
  echo "ninja $(ninja --version)"
  if command -v dpkg-query >/dev/null; then
    dpkg-query -W -f '${Package} ${Version}\n' 'gcc-mingw-w64*' 'binutils-mingw-w64*' 'mingw-w64*' 2>/dev/null | grep -v 'i686' | grep -Ev '^[^ ]+ $' || true
  fi
} > "$out/toolchain-$target.txt"

if [ "$target" = windows-x64 ]; then
  # Only DLLs every supported Windows ships may be imported.
  imports=$("$host-objdump" -p "$out/$binary" | awk '/DLL Name:/ { print $3 }')
  echo "DLL imports:" >> "$out/toolchain-$target.txt"
  echo "$imports" | sed 's/^/  /' >> "$out/toolchain-$target.txt"
  allowed='^(kernel32|user32|advapi32|bcrypt|gdi32|ole32|oleaut32|shell32|shlwapi|msvcrt|ucrtbase|ntdll|psapi|secur32|api-ms-win-[a-z0-9-]+)\.dll$'
  bad=$(echo "$imports" | tr 'A-Z' 'a-z' | grep -Ev "$allowed" || true)
  [ -z "$bad" ] || die "$binary imports non-system DLLs: $bad"
fi

for file in "$ffmpeg_tar" "$ffmpeg_tar.asc" "$opus_tar" "$dav1d_tar"; do
  [ -f "$work/downloads/$file" ] && cp "$work/downloads/$file" "$out/$file"
done
cp "$here/build.sh" "$here/configure-flags.txt" "$here/ffmpeg-release-key.asc" "$out/"
echo "ffmpeg-$FFMPEG_VERSION-oculus$OCULUS_BUILD" > "$out/release-tag.txt"
(cd "$out" && for f in *; do [ "$f" = SHA256SUMS ] || printf '%s  %s\n' "$(sha256 "$f")" "$f"; done > SHA256SUMS)

echo "built $out/$binary ($(wc -c < "$out/$binary" | tr -d ' ') bytes)"
cat "$out/toolchain-$target.txt"
