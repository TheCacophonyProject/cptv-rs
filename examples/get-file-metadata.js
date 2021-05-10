import {CptvDecoder} from "../index.js";
import {performance} from "perf_hooks";

(async function() {
  const start = performance.now();
  const file = "../cptv-files/20210429-201847.cptv";
  const decoder = new CptvDecoder();
  const metadata = await decoder.getFileMetadata(new URL(file, import.meta.url).pathname);
  decoder.close();
  const end = performance.now();
  console.log(`Time elapsed: ${end - start}ms`);
  console.log("Metadata", metadata);
  console.log("Duration (seconds)", metadata.duration);
})();
