import {CptvDecoder} from "../index.js";
import {performance} from "perf_hooks";

(async function() {
  const start = performance.now();
  const file = "../cptv-files/20210429-201847.cptv";
  const decoder = new CptvDecoder();
  await decoder.initWithCptvFile(new URL(file, import.meta.url).pathname);
  const header = await decoder.getHeader();
  const frames = [];
  while (!(await decoder.getTotalFrames())) {
    frames.push(await decoder.getNextFrame());
  }
  decoder.close();
  const end = performance.now();
  console.log(`Time elapsed: ${end - start}ms`);
  console.log("# Frames: ", frames.length);
  console.log("Header info", header);
})();
