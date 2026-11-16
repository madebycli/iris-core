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
3. Venice publishes no reference measurements to compare with. The user
   approves the runtime explicitly on first use (trust on first use); there is
   no built-in allowlist and no silent approval. The fingerprint covers MRTD and
   RTMR0-2 (the TD image and its boot chain). RTMR3 records per-instance runtime
   events, so it differs between replicas behind one model and is shown but not
   pinned. Approval does not audit the software or prove the model weights.
4. Pass that fingerprint to `complete`. A new attestation is checked each time;
   changed measurements require a new explicit approval before any prompt is sent.

`dcap-qvl` 0.7.1 verifies the Intel quote signature, production certificate chain,
revocation collateral, validity and TCB status locally with its RustCrypto
backend. Only non-debug TDX 1.0/1.5 reports with `UpToDate` TCB status are accepted.
The public Phala PCCS supplies Intel-signed collateral; it receives no Venice
credential or prompt. Fresh collateral is verified with the current system time.
Only `UpToDate` and `SWHardeningNeeded` TCB states pass. Iris checks the signing
key's Ethereum address against the first 20 REPORTDATA bytes, which is the only
binding the guide documents. The nonce must be echoed and, as the guide does not
document where it sits in REPORTDATA, either be found there (raw, SHA-256 of its
bytes or of its hex) or be confirmed by Venice's `server_verification.nonceBinding`
(as in the official CLI). `server_verification` is optional because only the CLI
uses it; a field that says `false` refuses the attestation.

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
native tool definitions are disabled. E2EE itself is switched on by the headers,
as the guide shows; the request carries no undocumented E2EE parameter. The frontend may interpret the existing
Iris planner text protocol only after authentication, with its usual local
validation and confirmations.

SSE is framed independently of network read boundaries and bounded in size.
Plaintext, malformed GCM, mixed completion IDs, content without an ID, incomplete
streams, tool-call deltas and missing `[DONE]` are errors. A reply that stops at
the token limit (`length`) is kept and marked truncated: it is complete as far as
it goes and still authenticated. Usage-only events may come without an ID. Reasoning
channels must also be encrypted and are never exposed. No partial result is
released to the caller.

After decryption Iris fetches `/tee/signature` (three short retries if it is not
stored yet, never a resend of the request) and verifies EIP-191/secp256k1 against
the attested key. The official CLI knows two signed texts: the reply content, or
`sha256(request):sha256(content)`. Accepted: the exact ciphertext (all deltas or
content deltas), a `request_hash:response_hash` pair whose request hash matches the
**exact transmitted JSON bytes** (response hash over the ciphertext, the raw SSE
body or the decrypted content), or the decrypted content itself only when the
signature names this completion's `request_id` (otherwise it could be a replay).
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

## Live result (November 2026, aci/1)

Checked against the live API with a real account (model `e2ee-gemma-4-26b-a4b-uncensored-p`):

- Attestation passes locally: Intel TDX quote via dcap-qvl and Phala PCCS, TCB `UpToDate`, key and nonce binding. The
  runtime fingerprint (MRTD, RTMR0-2) was the same across repeated requests.
- The encrypted request and the encrypted stream work; the reply decrypts with the per-request key.
- The signature endpoint now answers in Venice's `aci/1` format: `text` is `sha256(request):sha256(response)` as Venice's
  gateway saw them, plus a receipt with the chat id and an event log. Measured: the request hash was identical for two
  different encrypted requests, and the response hash does not match the stream Iris receives (Venice's API rewrites it
  after the gateway). Venice itself documents both hashes as provider-reported.
- Iris accepts such a signature only when the attested key signed it and both `requested_request_id` and
  `receipt.chat_id` name this request. That proves the attested runtime signed a record for this completion. It does not
  authenticate the reply's bytes; they stay confidential because they are encrypted to the per-request key.

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
