ffmpeg for Oculus on Windows x64 and macOS arm64

Version:  FFmpeg 8.1.3, build ffmpeg-8.1.3-oculus2 by the Oculus project.
          Unmodified upstream source; the build number counts our rebuilds.
Licence:  GNU Lesser General Public License, version 3 or later
          (configured with --enable-version3, without --enable-gpl or
          --enable-nonfree); see FFMPEG-LICENSE.txt
Linked:   libopus 1.6.1 (BSD 3-Clause) and dav1d 1.5.4 (BSD 2-Clause),
          statically. H.264 encoding is the operating system's own: Windows'
          Media Foundation (h264_mf) and macOS' VideoToolbox
          (h264_videotoolbox), which the program calls and does not contain.
          Otherwise only system libraries: Windows system DLLs, and on macOS
          libSystem and the VideoToolbox, CoreFoundation, CoreMedia,
          CoreVideo and CoreServices frameworks.
Source:   https://github.com/Tchanwangsa/oculus/releases/tag/ffmpeg-8.1.3-oculus2
          (see FFMPEG-SOURCE-OFFER.txt)
Script:   app/scripts/ffmpeg/build.sh and configure-flags.txt in
          https://github.com/Tchanwangsa/oculus, also attached to the release

SHA-256 of the upstream source tarballs, unmodified:

  7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3  ffmpeg-8.1.3.tar.xz
      (https://ffmpeg.org/releases/, signature checked against key
       FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8)
  6ffcb593207be92584df15b32466ed64bbec99109f007c82205f0194572411a1  opus-1.6.1.tar.gz
      (https://downloads.xiph.org/releases/opus/)
  686616b7c69eb88d44459391ab25cac13b6647a3b288835c5784e71c1514a5c5  dav1d-1.5.4.tar.xz
      (https://downloads.videolan.org/pub/videolan/dav1d/1.5.4/)

SHA-256 of the binaries as built:

  2f63039b98cb1afbb5d071933195f3e2e83dac1b80fd1d39b4407a534d1aaecb  ffmpeg.exe (Windows x64)
  202a0f68c33ca4ce89dd35ae80542484e9cfadf3363a7dde2a49616f25172906  ffmpeg (macOS arm64)

Windows x64: built on Ubuntu 24.04 with MinGW-w64 11.0.1 and GCC 13.2 (win32
threads), NASM 2.16.01 and Meson 1.3.2; the release's toolchain-windows-x64.txt
lists the exact packages. Rebuilding with "build.sh windows-x64 <out-dir>" on
the same toolchain reproduces ffmpeg.exe byte for byte.

macOS arm64: built natively on an Apple Silicon GitHub runner with Apple clang
21.0.0 and Meson 1.12.0, for macOS 13.3 or later; the release's
toolchain-macos-arm64.txt lists the versions. Rebuilding with "build.sh
macos-arm64 <out-dir>" builds the same program from the same sources; we do
not promise byte-identical output on macOS.

FFmpeg configuration, Windows x64 (ffmpeg -version; <work> is the build
machine's work directory):

  --prefix=<work>/prefix
  --enable-version3
  --disable-everything
  --disable-autodetect
  --disable-network
  --disable-doc
  --disable-ffplay
  --disable-ffprobe
  --disable-avdevice
  --disable-debug
  --enable-static
  --disable-shared
  --enable-protocol='file,pipe,fd'
  --enable-demuxer='mov,matroska,ogg,mp3,wav,w64,aiff,aac,ac3,eac3,flac,avi,mpegts,mpegps,asf'
  --enable-libdav1d
  --enable-decoder='h264,hevc,mpeg4,h263,mpeg1video,mpeg2video,msmpeg4v3,wmv1,wmv2,wmv3,vc1,mjpeg,vp8,vp9,theora,libdav1d'
  --enable-decoder='aac,aac_latm,mp3,mp3float,mp2,mp2float,opus,vorbis,flac,alac,ac3,eac3,wmav1,wmav2,wmapro'
  --enable-decoder='pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_f32le,pcm_f64le,pcm_u8,pcm_alaw,pcm_mulaw,adpcm_ima_wav,adpcm_ms'
  --enable-parser='h264,hevc,mpeg4video,mpegvideo,h263,vc1,mjpeg,vp8,vp9,av1,aac,aac_latm,mpegaudio,opus,vorbis,flac,ac3'
  --enable-libopus
  --enable-encoder='libopus,pcm_s16le,mjpeg,rawvideo'
  --enable-muxer='ogg,wav,mp4,mov,image2,rawvideo'
  --enable-demuxer='rawvideo,concat'
  --enable-decoder=rawvideo
  --enable-encoder=aac
  --enable-filter='scale,fps,format,aformat,aresample,null,anull,copy,trim,atrim,crop,transpose,hflip,vflip,rotate'
  --enable-bsf='null,aac_adtstoasc,extract_extradata,h264_mp4toannexb,hevc_mp4toannexb,vp9_superframe'
  --enable-cross-compile
  --target-os=mingw32
  --arch=x86_64
  --cross-prefix=x86_64-w64-mingw32-
  --enable-w32threads
  --disable-stripping
  --enable-mediafoundation
  --enable-d3d11va
  --enable-encoder=h264_mf
  --pkg-config=pkg-config
  --pkg-config-flags=--static
  --extra-cflags=-I<work>/prefix/include
  --extra-ldflags='-L<work>/prefix/lib -static -s -Wl,--no-insert-timestamp'

FFmpeg configuration, macOS arm64:

  --prefix=<work>/prefix
  --enable-version3
  --disable-everything
  --disable-autodetect
  --disable-network
  --disable-doc
  --disable-ffplay
  --disable-ffprobe
  --disable-avdevice
  --disable-debug
  --enable-static
  --disable-shared
  --enable-protocol='file,pipe,fd'
  --enable-demuxer='mov,matroska,ogg,mp3,wav,w64,aiff,aac,ac3,eac3,flac,avi,mpegts,mpegps,asf'
  --enable-libdav1d
  --enable-decoder='h264,hevc,mpeg4,h263,mpeg1video,mpeg2video,msmpeg4v3,wmv1,wmv2,wmv3,vc1,mjpeg,vp8,vp9,theora,libdav1d'
  --enable-decoder='aac,aac_latm,mp3,mp3float,mp2,mp2float,opus,vorbis,flac,alac,ac3,eac3,wmav1,wmav2,wmapro'
  --enable-decoder='pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_f32le,pcm_f64le,pcm_u8,pcm_alaw,pcm_mulaw,adpcm_ima_wav,adpcm_ms'
  --enable-parser='h264,hevc,mpeg4video,mpegvideo,h263,vc1,mjpeg,vp8,vp9,av1,aac,aac_latm,mpegaudio,opus,vorbis,flac,ac3'
  --enable-libopus
  --enable-encoder='libopus,pcm_s16le,mjpeg,rawvideo'
  --enable-muxer='ogg,wav,mp4,mov,image2,rawvideo'
  --enable-demuxer='rawvideo,concat'
  --enable-decoder=rawvideo
  --enable-encoder=aac
  --enable-filter='scale,fps,format,aformat,aresample,null,anull,copy,trim,atrim,crop,transpose,hflip,vflip,rotate'
  --enable-bsf='null,aac_adtstoasc,extract_extradata,h264_mp4toannexb,hevc_mp4toannexb,vp9_superframe'
  --target-os=darwin
  --arch=arm64
  --enable-videotoolbox
  --enable-encoder=h264_videotoolbox
  --pkg-config=pkg-config
  --pkg-config-flags=--static
  --extra-cflags=-I<work>/prefix/include
  --extra-ldflags='-L<work>/prefix/lib -Wl,-x'
