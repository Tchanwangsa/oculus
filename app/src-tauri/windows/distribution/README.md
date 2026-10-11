# Windows ffmpeg distribution

Both Windows packages ship `ffmpeg.exe` with three notices from this folder:

- `FFMPEG-LICENSE.txt`: the GNU LGPL v3 and GPL v3 texts from the FFmpeg
  tarball, then the `COPYING` files of libopus and dav1d, which are statically
  linked.
- `FFMPEG-README.txt`: version, upstream source hashes, configure line and
  where the build script lives.
- `FFMPEG-SOURCE-OFFER.txt`: the written offer of the corresponding source,
  which is the GitHub release `ffmpeg-8.1.3-oculus1` of this repository.

The binary is our own LGPL v3 build from `app/scripts/ffmpeg/build.sh`;
`.github/workflows/ffmpeg.yml` stages it, the byte-identical upstream
tarballs and the build files in that release as a draft. A maintainer
publishes the release, reviews the offer, then sets the repository variable
`OCULUS_WINDOWS_DISTRIBUTION_APPROVED` to `true`; until then the release
workflow retains installer artifacts for VM validation only. A build is not
legal approval.

These files describe one build. Once its release is published, any change to
the build (a version pin, the configure flags, the script) needs a new tag,
by `FFMPEG_VERSION` or `OCULUS_BUILD` in `build.sh`, and the README's hashes,
the offer's URL and `app/scripts/fetch-ffmpeg.mjs` change with it.

See [development.md](../../../../docs/development.md#windows-ffmpeg-is-our-own-lgpl-build)
for the build and [the packaging section](../../../../docs/development.md#windows-packaging-separates-build-artifacts-from-public-distribution)
for the Store identity variables and packaging commands.
