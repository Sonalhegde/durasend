use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::crypto;
use crate::manifest::{self, Manifest};
use crate::protocol;
use crate::relay;
use crate::upnp;

#[derive(Parser)]
#[command(
    name = "smartxfer",
    about = "Resilient, encrypted file transfer for unstable and remote networks",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a file to a receiver (direct connection or via relay)
    Send {
        /// Path to the file to transfer
        file: PathBuf,

        /// Direct receiver address (host:port)
        #[arg(long, conflicts_with = "relay")]
        to: Option<String>,

        /// Relay server address for NAT/firewall traversal (host:port)
        #[arg(long, conflicts_with = "to")]
        relay: Option<String>,

        /// Shared secret passphrase for AEAD encryption
        #[arg(long)]
        passphrase: String,

        /// Bytes per chunk (default: 1048576 = 1 MiB)
        #[arg(long, default_value_t = 1048576)]
        chunk_size: u32,

        /// Testing aid: deliberately drops connection partway through to test resume
        #[arg(long, default_value_t = false)]
        simulate_flaky: bool,
    },

    /// Run as a receiver (listening locally or connecting via relay)
    Receive {
        /// Bind address for direct connections (host:port)
        #[arg(long, conflicts_with = "relay")]
        listen: Option<String>,

        /// Relay server address for NAT/firewall traversal (host:port)
        #[arg(long, conflicts_with = "listen")]
        relay: Option<String>,

        /// Enable UPnP automatic router port mapping and external IP discovery
        #[arg(long, default_value_t = false)]
        upnp: bool,

        /// Directory where completed files and resume state live
        #[arg(long)]
        out_dir: PathBuf,

        /// Shared secret passphrase for AEAD decryption
        #[arg(long)]
        passphrase: String,
    },

    /// Run a lightweight, zero-knowledge TCP bridging relay
    Relay {
        /// Bind address for the relay (host:port)
        #[arg(long, default_value = "0.0.0.0:9099")]
        listen: String,
    },
}

pub fn run_cli(app_name: &str) -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send {
            file,
            to,
            relay,
            passphrase,
            chunk_size,
            simulate_flaky,
        } => run_send(&file, to.as_deref(), relay.as_deref(), &passphrase, chunk_size, simulate_flaky),
        Commands::Receive {
            listen,
            relay,
            upnp,
            out_dir,
            passphrase,
        } => run_receive(app_name, listen.as_deref(), relay.as_deref(), upnp, &out_dir, &passphrase),
        Commands::Relay { listen } => relay::run_relay(&listen),
    }
}

fn run_send(
    file_path: &Path,
    to: Option<&str>,
    relay_addr: Option<&str>,
    passphrase: &str,
    chunk_size: u32,
    simulate_flaky: bool,
) -> Result<()> {
    if !file_path.exists() {
        bail!("Source file does not exist: {:?}", file_path);
    }

    if to.is_none() && relay_addr.is_none() {
        bail!("You must specify either --to <ADDR> (direct) or --relay <ADDR> (remote relay)");
    }

    println!("[smartxfer] Preparing manifest for {:?}...", file_path);
    let manifest = manifest::create_manifest(file_path, chunk_size)
        .context("Failed to generate transfer manifest")?;
    println!(
        "[smartxfer] Manifest ready: file='{}', size={} bytes, chunks={}, file_id={}",
        manifest.file_name,
        manifest.file_size,
        manifest.chunk_hashes.len(),
        manifest.file_id
    );

    let key = crypto::derive_key(passphrase)?;

    let mut stream = if let Some(relay) = relay_addr {
        println!("[smartxfer] Connecting to relay at {}...", relay);
        let s = relay::connect_via_relay(relay, relay::ROLE_SENDER, passphrase)
            .context("Failed to connect and pair with receiver on relay")?;
        println!("[smartxfer] Paired with receiver via relay!");
        s
    } else {
        let dest = to.unwrap();
        println!("[smartxfer] Connecting to receiver at {}...", dest);
        let s = TcpStream::connect(dest)
            .with_context(|| format!("Failed to connect to receiver at {}", dest))?;
        println!("[smartxfer] Connected directly to receiver.");
        s
    };

    // Send manifest frame
    let manifest_bytes = serde_json::to_vec(&manifest)?;
    protocol::write_frame(&mut stream, &manifest_bytes)
        .context("Failed to send manifest frame")?;

    // Read missing chunks list from receiver
    let missing_bytes = protocol::read_frame(&mut stream)
        .context("Failed to read missing chunk indices from receiver")?;
    let missing_indices: Vec<u32> = serde_json::from_slice(&missing_bytes)
        .context("Failed to parse missing chunk list JSON")?;

    if missing_indices.is_empty() {
        println!("[smartxfer] Nothing to send -- receiver already has all chunks. Transfer complete.");
        return Ok(());
    }

    println!(
        "[smartxfer] Receiver requested {} missing chunk(s) out of {} total.",
        missing_indices.len(),
        manifest.chunk_hashes.len()
    );

    let flaky_threshold = if simulate_flaky {
        let threshold = (missing_indices.len() / 3).max(1);
        println!(
            "[smartxfer] --simulate-flaky enabled: will drop connection after sending {} chunk(s)",
            threshold
        );
        Some(threshold)
    } else {
        None
    };

    let mut source_file = File::open(file_path)
        .with_context(|| format!("Failed to open {:?}", file_path))?;
    let mut sent_count = 0;

    for &chunk_idx in &missing_indices {
        if let Some(limit) = flaky_threshold {
            if sent_count >= limit {
                println!(
                    "[smartxfer] [FLAKY SIMULATION] Intentionally killing connection after sending {} chunk(s).",
                    sent_count
                );
                let _ = stream.shutdown(std::net::Shutdown::Both);
                return Ok(());
            }
        }

        let offset = chunk_idx as u64 * manifest.chunk_size as u64;
        source_file.seek(SeekFrom::Start(offset))?;

        let bytes_to_read = if offset + manifest.chunk_size as u64 > manifest.file_size {
            (manifest.file_size - offset) as usize
        } else {
            manifest.chunk_size as usize
        };

        let mut raw_chunk = vec![0u8; bytes_to_read];
        source_file.read_exact(&mut raw_chunk)?;

        // Verify chunk hash before sending
        let local_hash = blake3::hash(&raw_chunk).to_hex().to_string();
        if local_hash != manifest.chunk_hashes[chunk_idx as usize] {
            bail!("Local chunk {} hash mismatch during read!", chunk_idx);
        }

        // Compress chunk
        let compressed = zstd::encode_all(&raw_chunk[..], 3)
            .context("Failed to compress chunk")?;

        // Encrypt chunk
        let ciphertext = crypto::encrypt_chunk(&compressed, &key)
            .context("Failed to encrypt chunk")?;

        // Format chunk frame: [index: u32 LE][ciphertext]
        let chunk_frame = protocol::encode_chunk_frame(chunk_idx, &ciphertext);

        // Transmit frame
        protocol::write_frame(&mut stream, &chunk_frame)
            .with_context(|| format!("Failed to send chunk {}", chunk_idx))?;

        sent_count += 1;
        println!(
            "[smartxfer] Sent chunk {}/{} (index: {}, raw: {} bytes, compressed+encrypted: {} bytes)",
            sent_count,
            missing_indices.len(),
            chunk_idx,
            raw_chunk.len(),
            ciphertext.len()
        );
    }

    println!("[smartxfer] All requested chunks sent successfully!");
    Ok(())
}

fn run_receive(
    app_name: &str,
    listen: Option<&str>,
    relay_addr: Option<&str>,
    enable_upnp: bool,
    out_dir: &Path,
    passphrase: &str,
) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create output directory {:?}", out_dir))?;

    if listen.is_none() && relay_addr.is_none() {
        bail!("You must specify either --listen <ADDR> (direct) or --relay <ADDR> (remote relay)");
    }

    let key = crypto::derive_key(passphrase)?;

    if let Some(relay) = relay_addr {
        println!("[smartxfer] Connecting to relay at {} (waiting for sender)...", relay);
        let mut stream = relay::connect_via_relay(relay, relay::ROLE_RECEIVER, passphrase)
            .context("Failed to connect and register on relay")?;
        println!("[smartxfer] Sender connected via relay! Receiving transfer...");

        handle_connection(&mut stream, out_dir, &key)?;
        println!("[smartxfer] Session complete.");
        return Ok(());
    }

    let listen_addr = listen.unwrap();

    // Handle UPnP if requested
    let _upnp_guard = if enable_upnp {
        let port = parse_port(listen_addr).unwrap_or(9099);
        println!("[upnp] Discovering local gateway router for port {}...", port);
        match upnp::UpnpMapping::try_map_port(port) {
            Ok(mapping) => {
                println!("[upnp] Router port {} successfully opened via UPnP!", port);
                if let Some(ext_ip) = &mapping.external_ip {
                    println!("[upnp] Public IP Address: {}", ext_ip);
                    println!(
                        "[upnp] Remote senders can transfer via: {} send <FILE> --to {}:{} --passphrase \"...\"",
                        app_name, ext_ip, port
                    );
                }
                Some(mapping)
            }
            Err(e) => {
                println!("[upnp] Note: UPnP router mapping unavailable ({}). Standard LAN listener active.", e);
                None
            }
        }
    } else {
        None
    };

    let listener = TcpListener::bind(listen_addr)
        .with_context(|| format!("Failed to bind receiver to {}", listen_addr))?;
    println!("[smartxfer] Receiver listening on {} (out_dir: {:?})", listen_addr, out_dir);

    for stream_res in listener.incoming() {
        let mut stream = match stream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[smartxfer] Connection accept error: {:?}", e);
                continue;
            }
        };

        let peer_addr = stream.peer_addr().ok();
        println!("[smartxfer] Accepted connection from {:?}", peer_addr);

        if let Err(e) = handle_connection(&mut stream, out_dir, &key) {
            eprintln!("[smartxfer] Transfer session error: {:?}", e);
        }
    }

    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    out_dir: &Path,
    key: &orion::aead::SecretKey,
) -> Result<()> {
    // 1. Read manifest frame
    let manifest_bytes = protocol::read_frame(stream)
        .context("Failed to receive manifest frame")?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .context("Failed to parse manifest JSON")?;

    println!(
        "[smartxfer] Received manifest: '{}' ({} bytes, {} chunks, file_id: {})",
        manifest.file_name,
        manifest.file_size,
        manifest.chunk_hashes.len(),
        manifest.file_id
    );

    // 2. Check if the final file is already fully assembled and valid
    let final_dest = out_dir.join(&manifest.file_name);
    if final_dest.exists() {
        if let Ok(meta) = fs::metadata(&final_dest) {
            if meta.len() == manifest.file_size {
                println!(
                    "[smartxfer] Target file '{}' already exists and matches expected size ({} bytes).",
                    manifest.file_name, manifest.file_size
                );
                let missing_indices: Vec<u32> = Vec::new();
                let missing_bytes = serde_json::to_vec(&missing_indices)?;
                protocol::write_frame(stream, &missing_bytes)?;
                return Ok(());
            }
        }
    }

    // 3. Identify temp directory for this transfer
    let temp_dir = out_dir.join(format!(".durasend_{}", manifest.file_id));
    fs::create_dir_all(&temp_dir)?;

    // Persist manifest
    let manifest_file_path = temp_dir.join("manifest.json");
    if !manifest_file_path.exists() {
        fs::write(&manifest_file_path, &manifest_bytes)?;
    }

    // 4. Scan temp dir for existing valid chunks
    let mut missing_indices = Vec::new();
    let total_chunks = manifest.chunk_hashes.len();

    for index in 0..total_chunks {
        let chunk_file_path = temp_dir.join(format!("chunk_{}", index));
        let mut is_valid = false;

        if chunk_file_path.exists() {
            if let Ok(mut chunk_file) = File::open(&chunk_file_path) {
                let mut data = Vec::new();
                if chunk_file.read_to_end(&mut data).is_ok() {
                    let h = blake3::hash(&data).to_hex().to_string();
                    if h == manifest.chunk_hashes[index] {
                        is_valid = true;
                    }
                }
            }
        }

        if !is_valid {
            missing_indices.push(index as u32);
        }
    }

    let already_have = total_chunks - missing_indices.len();
    println!(
        "[smartxfer] Resume check: {}/{} chunks already present and verified. Requesting {} missing chunks.",
        already_have,
        total_chunks,
        missing_indices.len()
    );

    // 5. Send missing chunk list
    let missing_bytes = serde_json::to_vec(&missing_indices)?;
    protocol::write_frame(stream, &missing_bytes)
        .context("Failed to send missing chunk indices to sender")?;

    if missing_indices.is_empty() {
        println!("[smartxfer] All chunks already verified. Ensuring final file is assembled.");
        assemble_file(out_dir, &temp_dir, &manifest)?;
        return Ok(());
    }

    // 6. Receive missing chunk frames
    let mut chunks_remaining = missing_indices.len();
    while chunks_remaining > 0 {
        let frame_payload = match protocol::read_frame(stream) {
            Ok(p) => p,
            Err(e) => {
                println!(
                    "[smartxfer] Transfer interrupted ({} chunks pending): {}",
                    chunks_remaining, e
                );
                return Ok(());
            }
        };

        let (index, ciphertext) = protocol::decode_chunk_frame(&frame_payload)
            .context("Failed to decode chunk frame")?;

        if (index as usize) >= total_chunks {
            bail!("Received invalid chunk index: {}", index);
        }

        // Decrypt
        let compressed = crypto::decrypt_chunk(ciphertext, key)
            .with_context(|| format!("Failed to decrypt chunk {}", index))?;

        // Decompress
        let plaintext = zstd::decode_all(compressed.as_slice())
            .with_context(|| format!("Failed to decompress chunk {}", index))?;

        // Verify BLAKE3 hash
        let actual_hash = blake3::hash(&plaintext).to_hex().to_string();
        let expected_hash = &manifest.chunk_hashes[index as usize];
        if actual_hash != *expected_hash {
            bail!(
                "Integrity check failed for chunk {}: expected {}, got {}",
                index,
                expected_hash,
                actual_hash
            );
        }

        // Write verified chunk atomically to disk
        let tmp_chunk_path = temp_dir.join(format!("chunk_{}.tmp", index));
        let final_chunk_path = temp_dir.join(format!("chunk_{}", index));

        {
            let mut chunk_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_chunk_path)?;
            chunk_file.write_all(&plaintext)?;
            chunk_file.flush()?;
        }
        fs::rename(&tmp_chunk_path, &final_chunk_path)?;

        chunks_remaining -= 1;
        println!(
            "[smartxfer] Verified and stored chunk {} ({} bytes plaintext). Chunks remaining: {}",
            index,
            plaintext.len(),
            chunks_remaining
        );
    }

    // 7. When all chunks are stored, assemble the final file
    assemble_file(out_dir, &temp_dir, &manifest)?;

    Ok(())
}

fn assemble_file(out_dir: &Path, temp_dir: &Path, manifest: &Manifest) -> Result<()> {
    let final_dest = out_dir.join(&manifest.file_name);
    let tmp_dest = out_dir.join(format!("{}.assembling", &manifest.file_name));

    // Verify all chunks are present before assembly
    for index in 0..manifest.chunk_hashes.len() {
        let chunk_path = temp_dir.join(format!("chunk_{}", index));
        if !chunk_path.exists() {
            bail!("Cannot assemble file: missing chunk_{}", index);
        }
    }

    println!("[smartxfer] Assembling complete file into {:?}...", final_dest);
    {
        let mut final_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_dest)?;

        for index in 0..manifest.chunk_hashes.len() {
            let chunk_path = temp_dir.join(format!("chunk_{}", index));
            let mut chunk_file = File::open(&chunk_path)?;
            std::io::copy(&mut chunk_file, &mut final_file)?;
        }
        final_file.flush()?;
    }

    // Verify assembled file size
    let assembled_size = fs::metadata(&tmp_dest)?.len();
    if assembled_size != manifest.file_size {
        bail!(
            "Assembled file size mismatch: expected {} bytes, got {}",
            manifest.file_size,
            assembled_size
        );
    }

    // Atomic move into final position
    if final_dest.exists() {
        fs::remove_file(&final_dest)?;
    }
    fs::rename(&tmp_dest, &final_dest)?;

    // Clean up temporary chunk files
    let _ = fs::remove_dir_all(temp_dir);

    println!(
        "[smartxfer] SUCCESS! File '{}' assembled successfully at {:?}",
        manifest.file_name, final_dest
    );
    Ok(())
}

fn parse_port(addr: &str) -> Option<u16> {
    if let Some(colon) = addr.rfind(':') {
        addr[colon + 1..].parse().ok()
    } else {
        addr.parse().ok()
    }
}
