#!/bin/sh
SYSROOT="$(rustc --print sysroot)"
HOST="$(rustc -vV | sed -n 's/host: //p')"
LLD="$SYSROOT/lib/rustlib/$HOST/bin/gcc-ld/ld64.lld"
exec cc "-fuse-ld=$LLD" "$@"
