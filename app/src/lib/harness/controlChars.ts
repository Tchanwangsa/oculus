/** C0 controls and DEL, except tab, line feed and carriage return. A key WebKit
 *  turns into text instead of a command (an arrow's legacy character is
 *  U+001D–U+001F) lands in the composer as one of these and draws as tofu. */
const CONTROL_RE = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g;

export const hasControlChars = (text: string): boolean => {
  CONTROL_RE.lastIndex = 0;
  return CONTROL_RE.test(text);
};

export const stripControlChars = (text: string): string => text.replace(CONTROL_RE, "");
