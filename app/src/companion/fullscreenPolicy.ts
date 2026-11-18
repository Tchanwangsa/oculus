type BrowserDevice = Pick<Navigator, 'userAgent' | 'platform' | 'maxTouchPoints'>;

/** iPadOS also sends a Mac desktop identity; touch points distinguish it.
 * Apple's native fullscreen dismissal gesture competes with whiteboard strokes.
 */
export function shouldUseNativeWhiteboardFullscreen(device: BrowserDevice): boolean {
  const appleMobile = /iPad|iPhone|iPod/i.test(device.userAgent);
  const desktopIdentityOnIpad = /Mac/i.test(device.platform) && device.maxTouchPoints > 1;
  return !appleMobile && !desktopIdentityOnIpad;
}
