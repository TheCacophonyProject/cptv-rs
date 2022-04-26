#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail
set -o xtrace

readonly TARGET_HOST=pi3
readonly TARGET_PATH=/home/pi/codec_test
#readonly TARGET_ARCH=armv7-unknown-linux-gnueabihf
readonly TARGET_ARCH=arm-unknown-linux-musleabihf
readonly SOURCE_PATH=../target/${TARGET_ARCH}/release/codec_test

cargo build --release --target=${TARGET_ARCH}
rsync ${SOURCE_PATH} ${TARGET_HOST}:${TARGET_PATH}
ssh -t ${TARGET_HOST} ${TARGET_PATH}
