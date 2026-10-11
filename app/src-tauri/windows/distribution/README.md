# Windows ffmpeg distribution

`FFMPEG-LICENSE.txt` and `FFMPEG-README.txt` are unmodified assets from
[ffmpeg-static b6.0](https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.0):
`win32-x64.LICENSE` and `win32-x64.README`. Both Windows packages include them.
The upstream README identifies the binary as Gyan's FFmpeg 6.0 essentials
build under GPL v3 and identifies FFmpeg source commit `ea3d24bbe3`.

The upstream source link is provenance, not a written source offer by Oculus
and not proof that all corresponding dependency sources are available. Before
public Windows distribution or Store submission, establish the exact
corresponding-source delivery for this binary and its linked libraries, place
the reviewed written offer in `FFMPEG-SOURCE-OFFER.txt` here, and set the
repository variable `OCULUS_WINDOWS_DISTRIBUTION_APPROVED` to `true`.
Do not create a placeholder offer or treat a successful build as legal approval.
The release workflow retains installer artifacts for VM validation while this
public-distribution gate is closed.

See [development.md](../../../../docs/development.md#windows-packaging-separates-build-artifacts-from-public-distribution)
for the Store identity variables and packaging commands.
