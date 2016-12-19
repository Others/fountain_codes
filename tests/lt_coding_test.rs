extern crate fountain_codes;
extern crate rand;

use fountain_codes::{Metadata, Client, Source, Encoder, Decoder, LtSource, LtClient};

#[test]
fn test_lt_coding_small() {
    let byte_count: usize = 100;

    let metadata = Metadata::new(byte_count as u64);
    let data = random_bytes(byte_count);

    let source: LtSource  = LtSource::new(metadata, data.clone()).unwrap();
    let mut client: LtClient = LtClient::new(metadata).unwrap();

    let packet = source.create_packet();
    println!("Packet {:?}", packet);

    println!("Client pre-packet {:?}", client);
    client.receive_packet(packet);
    println!("Client post-packet {:?}", client);

    let result = client.get_result().expect("One packet should be enough to transmit a small ammount of data...");
    assert_eq!(result, data);
}

// Bench:
//     When DEFAULT_FAILURE_PROBABILITY = 0.01 & DEFAULT_HINT_CONSTANT = 0.3
//         Finished after 21339 iterations
//     When DEFAULT_FAILURE_PROBABILITY = 0.1 & DEFAULT_HINT_CONSTANT = 0.3
//         Finished after 19533, 19680 iterations
#[test]
fn test_lt_coding_medium() {
    let byte_count: usize = 15 * 1024 * 1024;

    let metadata = Metadata::new(byte_count as u64);
    let data = random_bytes(byte_count);

    let source: LtSource  = LtSource::new(metadata, data).unwrap();
    let mut client: LtClient = LtClient::new(metadata).unwrap();

    // Going over a 100000 packets means the decoding almost certainly failed
    for _ in 0..100000 {
        let packet = source.create_packet();
        client.receive_packet(packet);

        println!("Decoding progress {}", client.decoding_progress());
        if client.get_result().is_some() {
            return;
        }
    }
    assert!(client.get_result().is_some());
}


fn random_bytes(byte_count: usize) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::with_capacity(byte_count);
    while result.len() < byte_count {
        result.push(rand::random());
    }
    result
}