import * as cptvPlayer from "./pkg/index.js";
/**
 * NOTE: For browser usage, these imports need to be stubbed
 *  out in your webpack config using:
 *
 * resolve: {
 *  fallback: {
 *    fs,
 *    module,
 *  }
 * }
 */


import fs from "fs/promises";
import { createRequire } from "module";

class Unlocker {
  constructor() {
    this.fn = null;
  }
  unlock() {
    this.fn && this.fn();
  }
}

// For use in nodejs to wrap an already loaded array buffer into a Reader interface
const FakeReader = function (bytes) {
  const state = {
    offsets: []
  };
  state.bytes = bytes;
  state.offset = 0;
  const length = bytes.byteLength;
  // How many reader chunks to split the file into
  const numParts = 100;
  const percentages = length / numParts;
  for (let i = 0; i < numParts; i++) {
    state.offsets.push(Math.ceil(percentages * i));
  }
  state.offsets.push(length);
  return {
    read() {
      return new Promise((resolve) => {
        state.offset += 1;
        resolve({
          value: state.bytes.slice(state.offsets[state.offset - 1], state.offsets[state.offset]),
          done: state.offset === numParts
        });
      });
    },
    cancel() {
      // Does nothing.
      return new Promise((resolve) => {
        resolve()
      });
    }
  }
};

// TODO(jon): This differs depending on whether the sensor is lepton 3 or 3.5
const AVERAGE_HEADROOM_OVER_BACKGROUND = 300;

export class CptvPlayer {

  // TODO(jon): Make this initialise in a worker as a constructor param.

  async initWithCptvUrlAndSize(url, size) {
    const unlocker = new Unlocker();
    await this.lockUncontended(unlocker);
    this.locked = true;
    if (!this.inited) {
      await cptvPlayer.default();
      cptvPlayer.CptvPlayerContext.init();
    } else {
      this.playerContext.free();
      this.reader && await this.reader.cancel();
    }
    this.playerContext = cptvPlayer.CptvPlayerContext.new();
    try {
      // Use this expired JWT token to test that failure case (usually when a page has been open too long)
      //const oldJWT = "https://api.cacophony.org.nz/api/v1/signedUrl?jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJfdHlwZSI6ImZpbGVEb3dubG9hZCIsImtleSI6InJhdy8yMDIxLzA0LzE1LzQ3MGU2YjY1LWZkOTgtNDk4Ny1iNWQ3LWQyN2MwOWIxODFhYSIsImZpbGVuYW1lIjoiMjAyMTA0MTUtMTE0MjE2LmNwdHYiLCJtaW1lVHlwZSI6ImFwcGxpY2F0aW9uL3gtY3B0diIsImlhdCI6MTYxODQ2MjUwNiwiZXhwIjoxNjE4NDYzMTA2fQ.p3RAOX7Ns52JqHWTMM5Se-Fn-UCyRtX2tveaGrRmiwo";
      this.response = await fetch(url);
      if (this.response.status === 200) {
        this.reader = this.response.body.getReader();
        this.expectedSize = size;
        await this.playerContext.initWithStream(this.reader, size);
        unlocker.unlock();
        this.inited = true;
        this.locked = false;
        return true;
      } else {
        this.locked = false;
        return (await this.response.json()).messages.pop();
      }
    } catch (e) {
      this.locked = false;
      return `Failed to load CPTV url ${url}`;
    }

  }

  async initWithCptvFile(filePath) {
    // Don't call this from a browser!
    const file = await fs.readFile(filePath);
    // Need to wrap file in
    const require = createRequire(import.meta.url);
    const path = require.resolve("./pkg/index_bg.wasm");
    const wasm = await fs.readFile(path);
    const unlocker = new Unlocker();
    await this.lockUncontended(unlocker);
    this.locked = true;
    if (!this.inited) {
      await cptvPlayer.default(wasm);
      cptvPlayer.CptvPlayerContext.init();
    } else {
      this.playerContext.free();
      this.reader && await this.reader.cancel();
    }
    this.playerContext = cptvPlayer.CptvPlayerContext.new();
    this.reader = new FakeReader(file);
    this.expectedSize = file.length;
    await this.playerContext.initWithStream(this.reader, file.length);
    unlocker.unlock();
    this.inited = true;
    this.locked = false;
  }

  async seekToFrame(frameNum) {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    const unlocker = new Unlocker();
    await this.lockUncontended(unlocker);
    this.locked = true;
    this.playerContext = await cptvPlayer.CptvPlayerContext.seekToFrame(this.playerContext, frameNum);
    unlocker.unlock();
    this.locked = false;
  }

  async lockUncontended(unlocker) {
    return new Promise((resolve, reject) => {
      if (this.locked) {
        unlocker.fn = resolve;
      } else {
        resolve();
      }
    });
  }

  async getHeader() {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    const unlocker = new Unlocker();
    await this.lockUncontended(unlocker);
    this.locked = true;
    this.playerContext = await cptvPlayer.CptvPlayerContext.fetchHeader(this.playerContext);
    unlocker.unlock();
    this.locked = false;
    return this.playerContext.getHeader();
  }

  getFrameAtIndex(frameNum) {
    const frameData = this.playerContext.getRawFrameN(frameNum);
    if (frameData.length === 0) {
      return null;
    }
    const min = this.playerContext.getMinValue();
    const max = Math.max(this.playerContext.getMaxValue(), min + AVERAGE_HEADROOM_OVER_BACKGROUND);
    // TODO(jon): Look into adaptive normalisation/histogram schemes
    return { min, max, data: frameData };
  }

  getTotalFrames() {
    if (this.inited && this.playerContext.ptr && this.playerContext.streamComplete()) {
      return this.playerContext.totalFrames();
    }
    return null;
  }

  getFrameHeaderAtIndex(frameNum) {
    const header = this.playerContext.getFrameHeader(frameNum);
    header.imageData.originalMax = header.imageData.max;
    header.imageData.max = Math.max(header.imageData.max, header.imageData.min + AVERAGE_HEADROOM_OVER_BACKGROUND);
    return header;
  }

  getBackgroundFrame() {
    const frameData = this.playerContext.getBackgroundFrame();
    if (frameData.length === 0) {
      return null;
    }
    const min = this.playerContext.getMinValue();
    const max = Math.max(this.playerContext.getMaxValue(), min + AVERAGE_HEADROOM_OVER_BACKGROUND);
    return { min, max, data: frameData };
  }

  getLoadProgress() {
    // This doesn't actually tell us how much has downloaded, just how much has been lazily read.
    return this.playerContext.bytesLoaded() / this.expectedSize;
  }
}
