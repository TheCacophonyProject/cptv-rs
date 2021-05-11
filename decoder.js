let CptvPlayerContext;

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
const FakeReader = function (bytes, maxChunkSize) {
  const state = {
    offsets: []
  };
  state.bytes = bytes;
  state.offset = 0;
  const length = bytes.byteLength;
  // How many reader chunks to split the file into
  let numParts = 5;
  if (maxChunkSize !== 0) {
    numParts = Math.ceil(length / maxChunkSize);
  }
  const percentages = length / numParts;
  for (let i = 0; i < numParts; i++) {
    state.offsets.push(Math.ceil(percentages * i));
  }
  state.offsets.push(length);
  return {
    read() {
      return new Promise((resolve) => {
        state.offset += 1;
        const value = state.bytes.slice(state.offsets[state.offset - 1], state.offsets[state.offset]);
        resolve({
          value,
          done: state.offset === state.offsets.length - 1
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
// TODO(jon): This is probably out of scope for this library, should be handled
//  at the player level.
let initedWasm = false;

export class CptvDecoderInterface {
  async initWithCptvUrlAndSize(url, size) {
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (!initedWasm) {
      CptvPlayerContext = (await import ("./pkg/index.js")).CptvPlayerContext;
      initedWasm = true;
    } else if (initedWasm && this.inited) {
      this.playerContext.free();
      this.reader && await this.reader.cancel();
    }
    try {
      // Use this expired JWT token to test that failure case (usually when a page has been open too long)
      // const oldJWT = "https://api.cacophony.org.nz/api/v1/signedUrl?jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJfdHlwZSI6ImZpbGVEb3dubG9hZCIsImtleSI6InJhdy8yMDIxLzA0LzE1LzQ3MGU2YjY1LWZkOTgtNDk4Ny1iNWQ3LWQyN2MwOWIxODFhYSIsImZpbGVuYW1lIjoiMjAyMTA0MTUtMTE0MjE2LmNwdHYiLCJtaW1lVHlwZSI6ImFwcGxpY2F0aW9uL3gtY3B0diIsImlhdCI6MTYxODQ2MjUwNiwiZXhwIjoxNjE4NDYzMTA2fQ.p3RAOX7Ns52JqHWTMM5Se-Fn-UCyRtX2tveaGrRmiwo";
      this.consumed = false;
      this.response = await fetch(url);
      if (this.response.status === 200) {
        this.reader = this.response.body.getReader();
        if (!size) {
          size = Number(this.response.headers.get("Content-Length")) || 0;
        }
        this.expectedSize = size;
        this.playerContext = await CptvPlayerContext.newWithStream(this.reader);
        unlocker.unlock();
        this.inited = true;
        this.locked = false;
        return true;
      } else {
        this.locked = false;
        try {
          const r = await this.response.json();
          return (r.messages && r.messages.pop()) || r.message || "Unknown error";
        } catch (e) {
          return await r.text();
        }
      }
    } catch (e) {
      this.locked = false;
      return `Failed to load CPTV url ${url}, ${e}`;
    }
  }

  async initWithCptvFile(filePath) {
    // Don't call this from a browser!
    const file = await fs.readFile(filePath);
    const require = createRequire(import.meta.url);
    const path = require.resolve("./pkg-node/index_bg.wasm");
    const wasm = await fs.readFile(path);
    return this.initWithFileBytes(file, filePath, wasm);
  }

  async initWithFileBytes(fileBytes, filePath = "", wasm) {
    // Don't call this from a browser!
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (!initedWasm) {
      if (typeof window === "undefined") {
        const require = createRequire(import.meta.url);
        CptvPlayerContext = require("./pkg-node").CptvPlayerContext;
      } else {
        CptvPlayerContext = (await import ("./pkg/index.js")).CptvPlayerContext;
      }
      initedWasm = true;
    } else if (initedWasm && this.inited) {
      this.playerContext.free();
      this.reader && await this.reader.cancel();
    }
    this.consumed = false;
    this.reader = new FakeReader(fileBytes, 100000);
    this.expectedSize = fileBytes.length;
    try {
      this.playerContext = await CptvPlayerContext.newWithStream(this.reader);
      unlocker.unlock();
      this.inited = true;
      this.locked = false;
      return true;
    } catch (e) {
      this.locked = false;
      return `Failed to load CPTV file ${filePath}, ${e}`;
    }
  }

  async fetchNextFrame() {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    if (this.consumed) {
      return "Stream has already been consumed and discarded";
    }
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (this.playerContext && this.playerContext.ptr) {
      this.playerContext = await CptvPlayerContext.fetchNextFrame(this.playerContext);
    }
    unlocker.unlock();
    this.locked = false;
    const frameData = this.playerContext.getNextFrame();
    if (frameData.length === 0) {
      return null;
    }
    const frameHeader = this.playerContext.getFrameHeader();
    return { data: new Uint16Array(frameData), meta: frameHeader };
  }

  async countTotalFrames() {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (this.playerContext && this.playerContext.ptr) {
      this.playerContext = await CptvPlayerContext.countTotalFrames(this.playerContext);
      // We can't call any other methods that read frame data on this stream,
      // since we've exhausted it and thrown away the data after scanning for the info we want.
      this.consumed = true;
    }
    unlocker.unlock();
    this.locked = false;
    return this.getTotalFrames();
  }

  async getMetadata() {
    const header = await this.getHeader();
    const totalFrameCount = await this.countTotalFrames();
    const duration = (1 / header.fps) * totalFrameCount;
    return {
      ...header,
      duration
    }
  }

  async getFileMetadata(filePath) {
    await this.initWithCptvFile(filePath, true);
    return await this.getMetadata();
  }

  async getStreamMetadata(url, size) {
    await this.initWithCptvUrlAndSize(url, size);
    return await this.getMetadata();
  }

  async lockIsUncontended(unlocker) {
    return new Promise((resolve) => {
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
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (this.playerContext && this.playerContext.ptr) {
      this.playerContext = await CptvPlayerContext.fetchHeader(this.playerContext);
    }
    const header = this.playerContext.getHeader();
    unlocker.unlock();
    this.locked = false;
    return header;
  }

  getTotalFrames() {
    if (!this.locked && this.inited && this.playerContext.ptr && this.playerContext.streamComplete()) {
      return this.playerContext.totalFrames();
    }
    return null;
  }

  getLoadProgress() {
    if (this.locked || (!this.playerContext || !this.playerContext.ptr)) {
      return null;
    }
    // This doesn't actually tell us how much has downloaded, just how much has been lazily read.
    return this.playerContext.bytesLoaded() / this.expectedSize;
  }
}
