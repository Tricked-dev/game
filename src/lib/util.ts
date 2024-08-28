export const base64ToArrayBuffer = (base64: string) => {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
};

export const arrayBufferToBase64 = (buffer: ArrayBuffer) => {
  const bytes = new Uint8Array(buffer);
  const binaryString = Array.prototype.map
    .call(bytes, (byte) => String.fromCharCode(byte))
    .join("");
  return btoa(binaryString).replace(/=+$/, ""); // Removes padding characters
};

export function shiftZerosDown(array: number[], width: number) {
  // Get the height of the array
  let height = Math.ceil(array.length / width);

  // Iterate over each column
  for (let col = 0; col < width; col++) {
    // For each column, iterate from the bottom to the top
    for (let row = height - 1; row >= 0; row--) {
      let index = row * width + col;

      if (array[index] === 0) {
        // Look upwards in the current column
        for (let checkRow = row - 1; checkRow >= 0; checkRow--) {
          let checkIndex = checkRow * width + col;
          if (array[checkIndex] !== 0) {
            // Swap the values
            array[index] = array[checkIndex];
            array[checkIndex] = 0;
            break;
          }
        }
      }
    }
  }

  return array;
}
