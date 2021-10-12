let encoder;
export const createTestCptvFile = async (params) => {
  if (!encoder) {
    encoder = await import("./encoder/pkg/encoder.js");
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
  return new Uint8Array(encoder.createTestCptvFile({...defaultParams, ...params}));
};
