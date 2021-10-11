import {CptvDecoder} from "../index.js";
import {createTestCptvFile} from "../encoder.js";


(async function() {
  const decoder = new CptvDecoder();
  const performance = typeof window === "undefined" ? (await import("perf_hooks")).performance : window.performance;
  const params = {
    duration: 5,
    fps: 1,
    hasBackgroundFrame: false,
    recordingDateTime: new Date().toISOString()
  };
  const s = performance.now();
  const file = await createTestCptvFile(params);
  console.log("Create test file", performance.now() - s, file.length);

  // Save the file for later?
  import("fs").then(({writeFileSync}) => {
    writeFileSync("test-no-bg.cptv", file);
  });

  // Optional: Verify the newly created file
  const start = performance.now();
  await decoder.initWithLocalCptvFile(new Uint8Array(file));
  const frames = [];
  let totalFrames = 0;
  let totalBackgroundFrames = 0;
  const header = await decoder.getHeader();
  while (!totalFrames && !(await decoder.hasStreamError())) {
    const frame = await decoder.getNextFrame();
    totalFrames = await decoder.getTotalFrames();
    if (totalFrames) {
      // Don't add the last frame twice.
      break;
    }
    if (frame) {
      if (frame.meta.isBackgroundFrame) {
        totalBackgroundFrames++;
      }
      frames.push(frame);
      console.log("Push frame", frames.length, frame.meta.timeOnMs);
    }
    console.log("Get total frames", totalFrames);
  }
  console.log("Total reported frames", totalFrames);
  console.log("Total reported background frames", totalBackgroundFrames);
  console.assert(!await decoder.hasStreamError());
  console.assert(totalFrames + totalBackgroundFrames === frames.length);
  const end = performance.now();
  console.log(`Time elapsed: ${end - start}ms`);
  console.log("# Frames: ", frames.length);
  console.log("Header info", header);
  await decoder.close();
}());
