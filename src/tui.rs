use anyhow::Result;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::crypto;
use crate::installer;
use crate::runner::{run_receive_action, run_send_action};

pub fn run_interactive_ui() -> Result<()> {
    loop {
        println!();
        println!("================================================================");
        println!("              SmartXfer — Resilient File Transfer               ");
        println!("       Autonomous, Encrypted, Resumable Transfer Suite          ");
        println!("================================================================");
        println!();
        println!("  [1] 📥 Receive a file (Direct LAN or UPnP)");
        println!("  [2] 📤 Send a file (Direct connection)");
        println!("  [3] 🌐 Receive via Remote Relay (Across firewalls / NAT)");
        println!("  [4] 🚀 Send via Remote Relay (Across firewalls / NAT)");
        println!("  [5] 🛠️  Start a Relay Server");
        println!("  [6] 📦 Install SmartXfer to System PATH");
        println!("  [7] 🧪 Run Cryptographic Self-Test & Diagnostic");
        println!("  [8] ❌ Exit");
        println!();
        print!("Select an option [1-8]: ");
        io::stdout().flush()?;

        let choice = read_input()?.trim().to_string();
        println!();

        match choice.as_str() {
            "1" => handle_direct_receive()?,
            "2" => handle_direct_send()?,
            "3" => handle_relay_receive()?,
            "4" => handle_relay_send()?,
            "5" => handle_start_relay()?,
            "6" => {
                installer::install_binary()?;
            }
            "7" => run_self_test()?,
            "8" | "q" | "exit" => {
                println!("Goodbye!");
                break;
            }
            _ => {
                println!("Invalid choice. Please select 1 through 8.");
            }
        }
    }

    Ok(())
}

fn handle_direct_receive() -> Result<()> {
    println!("--- 📥 Receive a File (Direct LAN / UPnP) ---");
    print!("Bind address [default: 0.0.0.0:9099]: ");
    io::stdout().flush()?;
    let mut listen = read_input()?.trim().to_string();
    if listen.is_empty() {
        listen = "0.0.0.0:9099".to_string();
    }

    print!("Save files into directory [default: ./received]: ");
    io::stdout().flush()?;
    let mut out_dir_str = read_input()?.trim().to_string();
    if out_dir_str.is_empty() {
        out_dir_str = "./received".to_string();
    }

    print!("Enable UPnP router port forwarding? (y/N): ");
    io::stdout().flush()?;
    let upnp_input = read_input()?.trim().to_lowercase();
    let upnp = upnp_input == "y" || upnp_input == "yes";

    print!("Shared secret passphrase: ");
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("Error: Passphrase cannot be empty.");
        return Ok(());
    }

    println!();
    run_receive_action(Some(&listen), None, upnp, &PathBuf::from(out_dir_str), &passphrase)?;
    Ok(())
}

fn handle_direct_send() -> Result<()> {
    println!("--- 📤 Send a File (Direct Connection) ---");
    print!("Path to file to send: ");
    io::stdout().flush()?;
    let file_str = read_input()?.trim().to_string();
    let file_path = PathBuf::from(&file_str);
    if !file_path.exists() {
        println!("Error: File {:?} does not exist.", file_path);
        return Ok(());
    }

    print!("Receiver address (host:port, e.g. 192.168.1.50:9099): ");
    io::stdout().flush()?;
    let to = read_input()?.trim().to_string();
    if to.is_empty() {
        println!("Error: Target address cannot be empty.");
        return Ok(());
    }

    print!("Shared secret passphrase: ");
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("Error: Passphrase cannot be empty.");
        return Ok(());
    }

    println!();
    run_send_action(&file_path, Some(&to), None, &passphrase, 1048576, false)?;
    Ok(())
}

fn handle_relay_receive() -> Result<()> {
    println!("--- 🌐 Receive via Remote Relay ---");
    print!("Relay server address [e.g. relay.example.com:9099]: ");
    io::stdout().flush()?;
    let relay = read_input()?.trim().to_string();
    if relay.is_empty() {
        println!("Error: Relay address cannot be empty.");
        return Ok(());
    }

    print!("Save files into directory [default: ./received]: ");
    io::stdout().flush()?;
    let mut out_dir_str = read_input()?.trim().to_string();
    if out_dir_str.is_empty() {
        out_dir_str = "./received".to_string();
    }

    print!("Shared secret passphrase: ");
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("Error: Passphrase cannot be empty.");
        return Ok(());
    }

    println!();
    run_receive_action(None, Some(&relay), false, &PathBuf::from(out_dir_str), &passphrase)?;
    Ok(())
}

fn handle_relay_send() -> Result<()> {
    println!("--- 🚀 Send via Remote Relay ---");
    print!("Path to file to send: ");
    io::stdout().flush()?;
    let file_str = read_input()?.trim().to_string();
    let file_path = PathBuf::from(&file_str);
    if !file_path.exists() {
        println!("Error: File {:?} does not exist.", file_path);
        return Ok(());
    }

    print!("Relay server address [e.g. relay.example.com:9099]: ");
    io::stdout().flush()?;
    let relay = read_input()?.trim().to_string();
    if relay.is_empty() {
        println!("Error: Relay address cannot be empty.");
        return Ok(());
    }

    print!("Shared secret passphrase: ");
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("Error: Passphrase cannot be empty.");
        return Ok(());
    }

    println!();
    run_send_action(&file_path, None, Some(&relay), &passphrase, 1048576, false)?;
    Ok(())
}

fn handle_start_relay() -> Result<()> {
    println!("--- 🛠️ Start a Relay Server ---");
    print!("Relay bind address [default: 0.0.0.0:9099]: ");
    io::stdout().flush()?;
    let mut listen = read_input()?.trim().to_string();
    if listen.is_empty() {
        listen = "0.0.0.0:9099".to_string();
    }

    println!();
    crate::relay::run_relay(&listen)?;
    Ok(())
}

fn run_self_test() -> Result<()> {
    println!("Running Cryptographic & Compression Diagnostic...");

    let test_data = b"SmartXfer high-entropy validation block for resilience and AEAD verification!";
    let passphrase = "diagnostic_secret_key_123";

    print!("  [1/4] Deriving AEAD key via BLAKE3... ");
    let key = crypto::derive_key(passphrase)?;
    println!("OK");

    print!("  [2/4] Compressing data with zstd level 3... ");
    let compressed = zstd::encode_all(&test_data[..], 3)?;
    println!("OK ({} -> {} bytes)", test_data.len(), compressed.len());

    print!("  [3/4] Encrypting with Orion XChaCha20-Poly1305 AEAD... ");
    let ciphertext = crypto::encrypt_chunk(&compressed, &key)?;
    println!("OK (ciphertext size: {} bytes)", ciphertext.len());

    print!("  [4/4] Decrypting, decompressing, and verifying roundtrip... ");
    let decrypted = crypto::decrypt_chunk(&ciphertext, &key)?;
    let decompressed = zstd::decode_all(decrypted.as_slice())?;
    assert_eq!(&decompressed, test_data);
    println!("OK (100% Match!)");

    println!("\n✅ All cryptographic subsystems are operating with 100% integrity!");
    Ok(())
}

fn read_input() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}
