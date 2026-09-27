# DuraSend — Architecture & Design Document

**Project:** Resilient, secure file transfer for unstable networks
**Component:** durasend CLI (Phase 1 of the Resilient File Transfer Suite)
**Status:** MVP — builds and runs, core resume/encryption path tested end-to-end
**Applications:** Rural clinics, disaster zones, racetrack telemetry

---

## 1. Problem Statement

Standard file transfer tools (scp, rsync over ssh, HTTP upload) assume a mostly-stable connection. On the networks this tool targets, that assumption is wrong:

| Environment | Network characteristic |
|---|---|
| Rural clinic | Intermittent satellite/cellular, hours of downtime |
| Disaster zone | Damaged infrastructure, ad-hoc mesh, no fixed IPs |
| Racetrack telemetry | High-frequency small updates, RF interference, many nodes |

The design constraint: **a transfer must survive the connection dying at any byte offset, and resuming must never mean starting over.**

### Requirements
1. **Resumable** — a dropped connection loses at most the in-flight chunk.
2. **Verifiable** — every chunk is integrity-checked independently with BLAKE3.
3. **Encrypted end-to-end** — XChaCha20-Poly1305 AEAD via Orion.
4. **Bandwidth-conscious** — zstd compress before encrypting.
5. **Infrastructure-light** — single static binary, no central server.

---

## 2. CLI Design

`ash
# Send
durasend send <FILE> --to <ADDR> --passphrase <PASS> [--chunk-size <BYTES>] [--simulate-flaky]

# Receive
durasend receive --listen <ADDR> --out-dir <DIR> --passphrase <PASS>
`

---

## 3. Project Structure

`
durasend/
├── Cargo.toml
├── ARCHITECTURE.md
└── src/
    ├── main.rs       CLI entry point, send/receive orchestration, assembly
    ├── manifest.rs   Manifest struct, file_id derivation, chunk hashing
    ├── crypto.rs     Key derivation, XChaCha20-Poly1305 AEAD encrypt/decrypt
    └── protocol.rs   Length-prefixed framing and binary chunk payload framing
`
