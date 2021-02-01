wasm-pack build --target no-modules --out-dir pkg-no-modules
echo a|Xcopy /E /I .\pkg-no-modules\cptv_player_bg.wasm ..\..\feverscreen\frontend\public\

echo a|Xcopy /E /I .\pkg-no-modules\*.ts ..\..\feverscreen\frontend\cptv-player\
echo a|Xcopy /E /I .\pkg-no-modules\package.json ..\..\feverscreen\frontend\cptv-player\
echo export default self.wasm_bindgen;>>.\pkg-no-modules\cptv_player.js
echo a|Xcopy /E /I .\pkg-no-modules\*.js ..\..\feverscreen\frontend\cptv-player\

REM Also build as --web and export to feverscreen.github.io
wasm-pack build --target web --out-dir pkg-web
echo a|Xcopy /E /I .\pkg-web\*.ts ..\..\feverscreen.github.io\cptv-player\
echo a|Xcopy /E /I .\pkg-web\*.json ..\..\feverscreen.github.io\cptv-player\
echo a|Xcopy /E /I .\pkg-web\*.js ..\..\feverscreen.github.io\cptv-player\
echo a|Xcopy /E /I .\pkg-web\*.wasm ..\..\feverscreen.github.io\cptv-player\

