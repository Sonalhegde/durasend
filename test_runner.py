import subprocess
import time
import os
import sys
import shutil

exe = os.path.abspath("target/release/durasend.exe")
source_file = os.path.abspath("test_env/source_data.bin")
out_dir = os.path.abspath("test_env/receiver_out")
addr = "127.0.0.1:9555"
passphrase = "telemetry_secret_key_2026"

# Ensure clean slate for receiver output
if os.path.exists(out_dir):
    shutil.rmtree(out_dir)
os.makedirs(out_dir, exist_ok=True)

print("=== 1. Starting persistent receiver ===")
receiver = subprocess.Popen(
    [exe, "receive", "--listen", addr, "--out-dir", out_dir, "--passphrase", passphrase],
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True
)

time.sleep(1.0)
if receiver.poll() is not None:
    print("Receiver failed to start:")
    print(receiver.communicate()[0])
    sys.exit(1)
print("Receiver is running.")

try:
    print("\n=== 2. Running flaky send (simulated network failure) ===")
    p1 = subprocess.run(
        [exe, "send", source_file, "--to", addr, "--passphrase", passphrase, "--simulate-flaky"],
        capture_output=True,
        text=True
    )
    print(p1.stdout)
    if "[FLAKY SIMULATION]" not in p1.stdout:
        print("FAILED: Simulation did not trigger flaky drop")
        sys.exit(1)

    time.sleep(1.0)
    temp_dirs = [d for d in os.listdir(out_dir) if d.startswith(".durasend_")]
    print(f"Temp directories found: {temp_dirs}")
    assert len(temp_dirs) > 0, "No temp directory found!"

    tpath = os.path.join(out_dir, temp_dirs[0])
    chunks = [f for f in os.listdir(tpath) if f.startswith("chunk_")]
    print(f"Partial chunks saved on receiver disk before resume: {sorted(chunks)}")
    assert len(chunks) == 2, f"Expected 2 partial chunks saved, got {len(chunks)}"

    print("\n=== 3. Resuming send (flaky flag omitted) ===")
    p2 = subprocess.run(
        [exe, "send", source_file, "--to", addr, "--passphrase", passphrase],
        capture_output=True,
        text=True
    )
    print(p2.stdout)
    if "All requested chunks sent successfully!" not in p2.stdout:
        print("FAILED: Resume send failed")
        sys.exit(1)

    time.sleep(1.0)
    assembled = os.path.join(out_dir, "source_data.bin")
    if not os.path.exists(assembled):
        print("FAILED: Assembled file not found!")
        sys.exit(1)

    with open(source_file, "rb") as f1, open(assembled, "rb") as f2:
        b1 = f1.read()
        b2 = f2.read()

    assert b1 == b2, "File contents mismatch!"
    print(f"VERIFIED: {len(b2)} bytes match source file EXACTLY (100% byte-for-byte integrity)!")

    print("\n=== 4. Re-sending when already complete ===")
    p3 = subprocess.run(
        [exe, "send", source_file, "--to", addr, "--passphrase", passphrase],
        capture_output=True,
        text=True
    )
    print(p3.stdout)
    assert "Nothing to send" in p3.stdout, "Expected 'Nothing to send'"
    print("VERIFIED: Clean exit with zero re-transmissions when file already exists.")

    print("\n***************************************************")
    print("*** ALL RESILIENCE AND INTEGRITY TESTS PASSED!  ***")
    print("***************************************************")

finally:
    receiver.terminate()
    try:
        receiver.wait(timeout=2)
    except:
        receiver.kill()
