/** A blob as base64 for IPC — a byte array would cross as a JSON list of
 *  numbers. Pure DOM, so the browser companion can import it without Tauri. */
export function blobToBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result);
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.onerror = () => reject(reader.error ?? new Error("could not read the image"));
    reader.readAsDataURL(file);
  });
}
