mod capture;
mod protocol;
mod replay;

fn main() {
    tracing_subscriber::fmt::init();
    // Parse arguments
    let args: Vec<String> = std::env::args().collect();
    let (interface, output_file) = match args.as_slice() {
        [_, iface, file] => (iface, file),
        _ => {
            // List available interfaces and exit
            println!("Available network interfaces:");
            match capture::list_interfaces() {
                Ok(interfaces) => {
                    for (name, ip) in interfaces {
                        println!("  - {} ({})", name, ip);
                    }
                }
                Err(e) => {
                    eprintln!("Error listing interfaces: {}", e);
                }
            }
            println!("\nUsage: {} <interface> <output_file>", args[0]);
            println!("Example: {} eth0 mtgo_capture.pcap", args[0]);
            std::process::exit(1);
        }
    };

    // Build BPF filter for MTGO traffic
    let filter = capture::build_bpf_filter();
    println!("Starting capture with filter: {}", filter);

    // Set up Ctrl+C handler
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
        println!("\nReceived Ctrl+C, stopping capture...");
    }).expect("Error setting Ctrl+C handler");

    // Create packet capture
    let mut pcap = match capture::PacketCapture::new(interface) {
        Ok(capture) => capture,
        Err(e) => {
            eprintln!("Error creating packet capture: {}", e);
            std::process::exit(1);
        }
    };

    // Set BPF filter
    if let Err(e) = pcap.set_filter(&filter) {
        eprintln!("Error setting BPF filter: {}", e);
        std::process::exit(1);
    }

    // Create packet dumper
    let mut dumper = match capture::PacketDumper::new(output_file) {
        Ok(dumper) => dumper,
        Err(e) => {
            eprintln!("Error creating packet dumper: {}", e);
            std::process::exit(1);
        }
    };

    println!("Capturing on interface {} to {}", interface, output_file);
    println!("Press Ctrl+C to stop");

    // Main capture loop
    let mut packet_count = 0u64;
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        match pcap.next_packet() {
            Ok(packet) => {
                packet_count += 1;
                if let Err(e) = dumper.write_packet(&packet) {
                    eprintln!("Error writing packet: {}", e);
                }
                // Log progress every 1000 packets
                if packet_count % 1000 == 0 {
                    println!("Captured {} packets", packet_count);
                }
            }
            Err(e) => {
                eprintln!("Error capturing packet: {}", e);
                break;
            }
        }
    }

    println!("Capture complete. Total packets: {}", packet_count);
}
