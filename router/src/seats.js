// Seat registration page. Credentials are sealed here for the ops repository's Actions public key
// with libsodium's crypto_box_seal, so the router only ever relays ciphertext.
(() => {
  'use strict';
  const nacl = self.nacl;
  const blake2b = self.golemBlake2b;
  // crypto_box_seal: fresh X25519 key pair, nonce = BLAKE2b-192(ephemeral pk || recipient pk),
  // output = ephemeral pk || crypto_box(message). The ephemeral secret is wiped afterwards.
  function seal(message, recipient, ephemeral = nacl.box.keyPair()) {
    if (!(recipient instanceof Uint8Array) || recipient.length !== nacl.box.publicKeyLength) throw new Error('invalid public key');
    const input = new Uint8Array(64);
    input.set(ephemeral.publicKey);
    input.set(recipient, 32);
    const context = blake2b.blake2bInit(nacl.box.nonceLength);
    blake2b.blake2bUpdate(context, input);
    const nonce = blake2b.blake2bFinal(context);
    const boxed = nacl.box(message, nonce, recipient, ephemeral.secretKey);
    ephemeral.secretKey.fill(0);
    const sealed = new Uint8Array(32 + boxed.length);
    sealed.set(ephemeral.publicKey);
    sealed.set(boxed, 32);
    return sealed;
  }
  function toBase64(bytes) {
    let text = '';
    for (let index = 0; index < bytes.length; index += 0x8000) text += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
    return btoa(text);
  }
  const fromBase64 = text => Uint8Array.from(atob(text), char => char.charCodeAt(0));
  self.golemSeal = seal;
  self.golemBase64 = {toBase64, fromBase64};
  if (typeof document === 'undefined') return;

  const csrf = document.querySelector('meta[name="golem-csrf"]').content;
  const status = document.getElementById('status');
  async function post(path, body) {
    const response = await fetch(path, {method: 'POST', credentials: 'same-origin', cache: 'no-store', headers: {'Content-Type': 'application/json', 'X-Golem-Csrf': csrf}, body: JSON.stringify(body)});
    if (!response.ok) throw new Error((await response.text()).slice(0, 200));
    return response.json();
  }
  for (const form of document.querySelectorAll('form.seat')) {
    form.addEventListener('submit', async event => {
      event.preventDefault();
      const field = form.elements.credential;
      const provider = form.dataset.provider;
      const message = new TextEncoder().encode(field.value.trim());
      field.value = '';
      if (message.length === 0) return;
      status.textContent = `Encrypting your ${provider} sign-in…`;
      try {
        const response = await fetch('/v1/seats/key', {credentials: 'same-origin', cache: 'no-store'});
        if (!response.ok) throw new Error((await response.text()).slice(0, 200));
        const key = await response.json();
        const sealed = seal(message, fromBase64(key.key));
        const result = await post('/v1/seats', {provider, key_id: key.key_id, encrypted_value: toBase64(sealed)});
        status.textContent = `${result.replaced ? 'Replaced' : 'Registered'} your ${provider} seat. Reload to see it listed.`;
      } catch (error) {
        status.textContent = `Registration failed: ${error.message}`;
      } finally {
        message.fill(0);
      }
    });
  }
  const preferred = document.getElementById('preferred');
  preferred?.addEventListener('submit', async event => {
    event.preventDefault();
    const provider = preferred.elements.provider.value;
    try {
      await post('/v1/seats/preferred', {provider});
      status.textContent = `Preferred seat is now ${provider}.`;
    } catch (error) {
      status.textContent = `Saving the preferred seat failed: ${error.message}`;
    }
  });
})();
