# ffmpeg distribution

Both packages (the macOS `.app` and the Windows installer) ship ffmpeg with
three notices from this folder, and Oculus's own `LICENSE`:

- `FFMPEG-LICENSE.txt`: the GNU LGPL v3 and GPL v3 texts from the FFmpeg
  tarball, then the `COPYING` files of libopus and dav1d, which are statically
  linked.
- `FFMPEG-README.txt`: versions, upstream source hashes, each binary's hash
  and configure line, and where the build script lives.
- `FFMPEG-SOURCE-OFFER.txt`: the written offer of the corresponding source,
  which is the GitHub release `ffmpeg-8.1.3-oculus2` of this repository.

The binaries are our own LGPL v3 builds from `app/scripts/ffmpeg/build.sh`;
`.github/workflows/ffmpeg.yml` stages them, the byte-identical upstream
tarballs and the build files in that release as a draft. A maintainer
publishes the release and reviews the offer. For Windows, the maintainer then
sets the repository variable `OCULUS_WINDOWS_DISTRIBUTION_APPROVED` to `true`;
until then the release workflow retains installer artifacts for VM validation
only. A build is not legal approval.

These files describe one build. Once its release is published, any change to
the build (a version pin, the configure flags, the script) needs a new tag,
by `FFMPEG_VERSION` or `OCULUS_BUILD` in `build.sh`, and the README's hashes,
the offer's URL and `app/scripts/fetch-ffmpeg.mjs` change with it.

See [development.md](../../../docs/development.md#ffmpeg-is-our-own-lgpl-build)
for the build and [the packaging section](../../../docs/development.md#windows-packaging-separates-build-artifacts-from-public-distribution)
for the Store identity variables and packaging commands.
