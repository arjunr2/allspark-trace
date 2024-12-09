#!/bin/bash
rm -r inst; mkdir -p inst
make -C ../wasm-instrument
for f in orig/*.wasm; do
  x=$(basename $f .wasm)
  echo "Generating instrument binary for $f"
  ../wasm-instrument/instrument --scheme allspark-trace -o inst/$x.inst.wasm $f
  wasm2wat --enable-threads inst/$x.inst.wasm -o inst/$x.inst.wat
done
