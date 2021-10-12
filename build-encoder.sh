#npm run build-webpack
#npm run build-node
#cp ./pkg/package_fixed.json ./pkg/package.json
#wasm-opt -Oz ./pkg/index_bg.wasm -o ./pkg/index_bg.wasm


wasm-pack build ./encoder/ --out-name encoder
wasm-pack build ./encoder/ --target nodejs --out-name encoder-node
mv ./encoder/pkg/encoder-node.js ./encoder/pkg/encoder-node.cjs
wasm-opt -Oz ./encoder/pkg/encoder_bg.wasm -o ./encoder/pkg/encoder_bg.wasm
wasm-opt -Oz ./encoder/pkg/encoder-node_bg.wasm -o ./encoder/pkg/encoder-node_bg.wasm
