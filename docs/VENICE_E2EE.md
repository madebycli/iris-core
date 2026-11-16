# Venice encrypted text inference

`iris-cloud` owns the Venice protocol and is exported by `iris-ffi` as
`VeniceConnection`. Frontends supply HTTPS transport and their platform's secret
storage. Credentials do not enter Rust, model prompts, diagnostic strings, or
collateral requests. The local assistant and its offline classifier are unchanged.

The default is Gemma 4 26B A4B Uncensored,
`e2ee-gemma-4-26b-a4b-uncensored-p`. Every operation rechecks the live `/models`
catalog; both `supportsE2EE` and `supportsTeeAttestation` must be true. A model
name alone is not evidence of encryption support.

## Setup and trust

1. Obtain an API key in the Venice account's API settings. The frontend owns
   account login and billing consent; this is API-key authentication, not a new
   Iris password or an undocumented OAuth flow.
2. Call `models`, then `inspect(model)`. Inspection sends no conversation.
3. Compare the returned runtime fingerprint with an independently trusted
   publication or administrator. Explicitly approve it in the frontend. There
   is no built-in, invented measurement allowlist and no automatic first-use
   approval. Approval trusts that runtime's software; it does not audit it or
   prove its model weights.
4. Pass that fingerprint to `complete`. A new attestation is checked each time;
   changed measurements require a new explicit approval before any prompt is sent.

`dcap-qvl` 0.7.1 verifies the Intel quote signature, production certificate chain,
revocation collateral, validity and TCB status locally with its RustCrypto
backend. Only non-debug TDX 1.0/1.5 reports with `UpToDate` TCB status are accepted.
The public Phala PCCS supplies Intel-signed collateral; it receives no Venice
credential or prompt. Fresh collateral is verified with the current system time.
Iris checks the signing key's Ethereum address against the first 20 REPORTDATA
bytes, and the complete 32-byte client nonce (raw, SHA-256 of decoded bytes, or
SHA-256 of its hex representation) in the remaining signed REPORTDATA. Providers
with other layouts are rejected, even if Venice returns `verified: true`.

NVIDIA GPU attestation still relies on Venice's `server_verification.nvidia`
result when GPU evidence is present. Iris does **not** claim independent NVIDIA
verification, proof of particular model weights, or an audited inference runtime.
The frontend must disclose these trust boundaries before runtime approval.

## Request and reply

Each request has a fresh in-memory secp256k1 response key. Each message has its
own ephemeral key and random 96-bit AES-GCM nonce. RustCrypto performs ECDH,
HKDF-SHA256 (`ecdsa_encryption`) and AES-256-GCM. The wire envelope is:

```
uncompressed ephemeral public key (65) || nonce (12) || ciphertext || tag (16)
```

The hex envelope is sent with Venice's three `X-Venice-TEE-*` headers. Both
system and user content are encrypted. Assistant history is included **inside**
the encrypted user context because Venice's protocol decrypts only system and
user wire roles. Search, uploads, server memory, provider system prompts and
native tool definitions are disabled. The frontend may interpret the existing
Iris planner text protocol only after authentication, with its usual local
validation and confirmations.

SSE is framed independently of network read boundaries and bounded in size.
Plaintext, malformed GCM, mixed completion IDs, wrong models, incomplete streams,
tool-call deltas, token truncation and missing `[DONE]` are errors. Reasoning
channels must also be encrypted and are never exposed. No partial result is
released to the caller.

After decryption Iris fetches `/tee/signature` and verifies EIP-191/secp256k1
against the attested key. It requires either a signature of the exact ciphertext
or a signed `request_hash:response_hash` pair whose request hash matches the
**exact transmitted JSON bytes**. Supported response hash representations are
the encrypted content, the raw SSE body, and the decrypted content. A standalone
plaintext signature is insufficient: without request binding it permits replay.
Other provider signature/canonicalization formats fail closed and need a
reviewed protocol update; they are never treated as a successful encrypted chat.

There is no plaintext fallback, provider switch, automatic measurement update,
TLS bypass, test root in production, or retry of a charged completion. Host
transport must disable redirects, authenticate only the fixed Venice origin,
enforce the 8 MiB response cap while reading, and propagate cancellation.

## Verification and limitations

```
cargo test -p iris-cloud --locked
cargo clippy -p iris-cloud -p iris-ffi --all-targets --no-deps --locked -- -D warnings
```

Tests include an independent Python `cryptography` wire vector, an Intel-rooted
quote/expiry/tampering fixture, replay and model rejection, ciphertext-only
history, stream truncation, GCM manipulation, response signer and request binding.
The TDX fixture is from dcap-qvl 0.7.1, MIT (license beside the fixture). Its
historical verification time is fixed only in the test; production uses wall time.

A real Venice account, compatible attestation/response-signature format, an
independently approved runtime fingerprint, and API access are still required
for a live model roundtrip. Offline tests do not establish live compatibility or
an independent audit of this integration. Required HTTPS origins are
`api.venice.ai` and `pccs.phala.network`.

## Live test with a real account

Offline tests cannot prove that Venice's live formats match. Do this once with your own API key.
The key stays on the phone; nothing below needs it in a file, a chat or an issue.

In the app (Settings → AI provider → Venice):

1. Enter the API key and run **Verify connection and enclave**. Expected: an E2EE model list and
   the approval dialog with MRTD and RTMR0–2. Approve.
2. Ask a knowledge question ("Warum ist der Himmel blau?"). Expected: an answer, no error text.
3. Ask for a very long answer ("Erkläre ausführlich die Geschichte Roms"). Expected: an answer, possibly cut short, never an error (the core keeps a cut-off reply; the app does not mark it yet).
4. "Wie spät ist es?" and "äh hm": answered offline, no Venice usage in the Venice dashboard.
5. "Erinnere mich morgen um 9 an zahnarzt termin machen": with confirmation on, Iris proposes a
   clean title and a list, "ja" applies it, and the reminder still rings at 9.
6. "Ich heiße Alex", a new session later "Weißt du noch wie ich heiße?": answered from the memory page.
7. Forget the approved runtime and verify again: the dialog must appear again.

If a step fails, capture without the key (`export VENICE_API_KEY=...` in your own shell only):

```
curl -sS -H "Authorization: Bearer $VENICE_API_KEY" \
  "https://api.venice.ai/api/v1/tee/attestation?model=<model>&nonce=$(openssl rand -hex 32)" > attestation.json
curl -sS -H "Authorization: Bearer $VENICE_API_KEY" "https://api.venice.ai/api/v1/models" > models.json
```

Share only the field names and structure of `attestation.json` (the quote itself is public evidence,
but check it holds no account data), the app's error text and the step number. Never share the
key, request headers or decrypted answers.

Protocol sources: [Venice E2EE guide](https://docs.venice.ai/guides/features/tee-e2ee-models),
[official CLI](https://github.com/veniceai/venice-cli),
[DCAP verifier](https://github.com/Phala-Network/dcap-qvl).
