
/**
 * Helper function for rendering a raw frame into an Rgba destination buffer
 * @param targetFrameBuffer (Uint8ClampedArray) - destination frame buffer.  Must be width * height * 4 length
 * @param frame (Uint16Array) - Source raw frame of width * height uint16 pixels
 * @param colourMap (Uint32Array) Array of Rgba colours in uin32 form for mapping into 0..255 space
 * @param min (number) min value to use for normalisation
 * @param max (number) max value to use for normalisation
 */
export function renderFrameIntoFrameBuffer(
    targetFrameBuffer: Uint8ClampedArray,
    frame: Uint16Array,
    colourMap: Uint32Array,
    min: number,
    max: number
): void;

/**
 * Get the frame index at a given time offset, taking into account the presence of a background frame.
 * @param time {Number}
 * @param duration {Number}
 * @param fps {Number}
 * @param totalFramesIncludingBackground {Number}
 * @param hasBackgroundFrame {Boolean}
 */
export function getFrameIndexAtTime(
    time: number,
    duration: number,
    fps: number,
    totalFramesIncludingBackground: number | false,
    hasBackgroundFrame: boolean
): number;

/**
 * Default Colour maps to use for rendering frames on both front-end and back-end.
 */
export const ColourMaps: readonly [string, Uint32Array][];

