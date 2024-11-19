#!/usr/bin/env bash

export MIPS_CC=/Users/marijn/temp/asm-processor/ido-7.1-recomp-macos/cc

for A in tests/*.c tests/*.p; do
    OBJDUMPFLAGS=-srt
    echo $A
    ./compile-test-rust.sh "$A" && mips-linux-gnu-objdump $OBJDUMPFLAGS "${A%.*}.o" | diff -w "${A%.*}.objdump" - || echo FAIL "$A"
done
