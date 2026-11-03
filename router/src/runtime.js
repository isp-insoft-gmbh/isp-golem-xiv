// Native Workers WebCrypto: no wasm clock, RSA bigint implementation, or Node API.
const encoder = new TextEncoder();
// Timer handles are deliberately opaque: Workers can return numbers or objects.
export function makeDeadline(milliseconds) {
  let handle;
  const promise = new Promise(resolve => { handle = setTimeout(resolve, milliseconds); });
  return {promise, handle};
}
export function deadlinePromise(deadline) { return deadline.promise; }
export function cancelDeadline(deadline) { clearTimeout(deadline.handle); }
const b64url = bytes => btoa(String.fromCharCode(...bytes)).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
function der(tag, bytes) {
  const size = bytes.length;
  const length = size < 128 ? [size] : size < 256 ? [0x81, size] : [0x82, size >> 8, size & 255];
  return new Uint8Array([tag, ...length, ...bytes]);
}
export async function appJwt(pem) {
  if (pem.length > 16384) throw new Error('key limit');
  const pkcs1 = pem.includes('-----BEGIN RSA PRIVATE KEY-----');
  const encoded = pem.replace(/-----[^-]+-----/g, '').replace(/\s/g, '');
  let bytes = Uint8Array.from(atob(encoded), char => char.charCodeAt(0));
  if (pkcs1) {
    const rsaIdentifier = [0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00];
    bytes = der(0x30, [0x02, 0x01, 0x00, ...rsaIdentifier, ...der(0x04, bytes)]);
  }
  const key = await crypto.subtle.importKey('pkcs8', bytes, {name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256'}, false, ['sign']);
  const now = Math.floor(Date.now() / 1000);
  const header = b64url(encoder.encode(JSON.stringify({alg: 'RS256', typ: 'JWT'})));
  const claims = b64url(encoder.encode(JSON.stringify({iat: now - 60, exp: now + 540, iss: '4605060'})));
  const message = `${header}.${claims}`;
  const signature = await crypto.subtle.sign('RSASSA-PKCS1-v1_5', key, encoder.encode(message));
  return `${message}.${b64url(new Uint8Array(signature))}`;
}
