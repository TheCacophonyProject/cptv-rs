import {CptvDecoder} from "../index.js";
import {performance} from "perf_hooks";
import fs from "fs";

(async function() {
  const start = performance.now();
  const file = "../cptv-files/20200130-031836.cptv";
  const decoder = new CptvDecoder();
  const metadata = await decoder.getFileMetadata(new URL(file, import.meta.url).pathname);
  {
    // NOTE: Testing truncated files
    // const fileBytes = fs.readFileSync(new URL(file, import.meta.url).pathname);
    // const metadata = await decoder.getBytesMetadata(fileBytes.slice(0, 100000));
  }
  const fileIsCorrupt = await decoder.hasStreamError();
  if (fileIsCorrupt) {
    console.log(await decoder.getStreamError());
  }
  decoder.close();
  const end = performance.now();

  console.log(`Time elapsed: ${end - start}ms`);
  if (!fileIsCorrupt) {
    console.log("Metadata", metadata);
    console.log("Duration (seconds)", metadata.duration);
    console.log("Total frames (seconds)", metadata.totalFrames);
  } else {
    console.log("File is corrupt", metadata);
  }
})();
