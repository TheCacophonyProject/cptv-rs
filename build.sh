#npm run build-webpack
#npm run build-node
#cp ./pkg/package_fixed.json ./pkg/package.json
#wasm-opt -Oz ./pkg/index_bg.wasm -o ./pkg/index_bg.wasm


wasm-pack build ./decoder/ --out-name decoder
wasm-pack build ./decoder/ --target nodejs --out-name decoder-node
mv ./decoder/pkg/decoder-node.js ./decoder/pkg/decoder-node.cjs
wasm-opt -Oz ./decoder/pkg/decoder_bg.wasm -o ./decoder/pkg/decoder_bg.wasm
wasm-opt -Oz ./decoder/pkg/decoder-node_bg.wasm -o ./decoder/pkg/decoder-node_bg.wasm
