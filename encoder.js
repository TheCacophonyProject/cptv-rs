import init, { createTestCptvFile as create } from "./encoder/pkg/encoder.js";
import fs from "fs/promises";
let initedWasm = false;
export const createTestCptvFile = async (params) => {
  if (typeof window !== "undefined") {
    console.error("This should only be used from within a nodejs context");
    return;
  }
  if (!initedWasm) {
    const wasm = await fs.readFile("./encoder/pkg/encoder_bg.wasm");
    await init(wasm);
    initedWasm = true;
  }
  if (params.recordingDateTime && typeof params.recordingDateTime === 'object') {
    params.recordingDateTime = new Date(params.recordingDateTime).toISOString();
  }
  const defaultParams = {
    recordingDateTime: new Date().toISOString(),
    deviceName: "Test device",
    deviceId: 99999,
    brand: "Acme",
    model: "lo-res-20",
    serialNumber: 1234,
    firmwareVersion: "1.0",
    latitude: 1,
    duration: 10,
    longitude: 1,
    fps: 1,
    hasBackgroundFrame: true,
  };
  return new Uint8Array(create({...defaultParams, ...params}));
};
