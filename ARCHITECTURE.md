# DuraSend / SmartXfer — Architecture & Design Document

**Project:** Resilient, secure file transfer for unstable and remote networks  
**Component:** `smartxfer` / `durasend` CLI  
**Status:** MVP + Remote Network Extensions (UPnP & Encrypted Relay)  
**Applications:** Rural clinics, disaster zones, racetrack telemetry, remote cross-network synchronization  

---

## 1. Problem Statement

Standard file transfer tools (`scp`, `rsync` over ssh, HTTP upload) assume a mostly-stable connection. On the networks this tool targets, that assumption is wrong:

| Environment | Network characteristic |
|---|---|
| Rural clinic | Intermittent satellite/cellular, hours of downtime |
| Disaster zone | Damaged infrastructure, ad-hoc mesh, no fixed IPs |
| Racetrack telemetry | High-frequency small updates, RF interference, many nodes |
| Cross-network field ops | Carrier-grade NAT (CGNAT), double firewalls, no public static IPs |

The design constraint that drives every decision: **a transfer must survive the connection dying at any byte offset, and resuming must never mean starting over.**

---

## 2. Remote Network Traversal Architecture

To enable transfers across different remote networks without requiring static public IPs:

### 2.1 UPnP IGD Port Mapping (Direct P2P Option)
When the receiver runs with `--upnp`:
- Discovers the local router via SSDP UDP multicast (`239.255.255.250:1900`).
- Sends SOAP XML to `WANIPConnection` / `WANPPPConnection` requesting a temporary TCP port mapping.
- Fetches the router's external public IP address.
- When the receiver process terminates, it sends `DeletePortMapping` to release the port.

### 2.2 End-to-End Encrypted Relay (Firewall Traversal Option)
When devices are behind restrictive cellular, corporate, or double-NAT networks:
- A public host runs `smartxfer relay --listen 0.0.0.0:9099`.
- Both sender and receiver establish outgoing connections to the relay.
- Each client sends a 17-byte handshake: `[u8 role][16-byte session_token]`, where `session_token = blake3("smartxfer_relay_session_v1:" + passphrase)[0..16]`.
- The relay pairs the two sockets and bridges bytes bi-directionally.
- **Threat Model & Security:** The relay only sees opaque ciphertext. Because every chunk is AEAD-encrypted with Orion (XChaCha20-Poly1305), the relay has zero access to encryption keys and cannot inspect, decrypt, or tamper with file contents.

---

## 3. Core Resilience & Resume Design

### 3.1 Content-addressed resume
State is derived from file identity (`file_id = hash(name, size, chunk_size)`). The receiver maintains:
`<out_dir>/.durasend_<file_id>/`
containing verified chunk files (`chunk_0`, `chunk_1`, ...).
- Resume flow queries the directory, and only missing chunk indices are requested.
- Dropped links preserve existing chunks. Re-invoking the command resumes immediately.

### 3.2 Compress-then-encrypt
Chunks are compressed with zstd before AEAD encryption to maximize bandwidth efficiency without losing entropy.

---

## 4. Module Map

```
src/
├── lib.rs              Library root
├── runner.rs           CLI commands (send, receive, relay) and orchestration
├── manifest.rs         File manifest, chunk hashing, file_id derivation
├── crypto.rs           AEAD encrypt/decrypt (orion) & key derivation
├── protocol.rs         Framing layer ([length][payload], chunk framing)
├── relay.rs            TCP bridging relay server & client handshake
├── upnp.rs             UPnP IGD port forwarder & public IP query
└── bin/
    ├── smartxfer.rs    smartxfer executable
    └── durasend.rs     durasend executable
```
