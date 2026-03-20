use ferrum_crypt4gh::{recipient_keys_from_pubkey, C4ghKeys};
use std::collections::HashSet;

/// Learned from crypt4gh edit-list spec + production interoperability issues:
/// Edit-list offsets refer to PLAINTEXT coordinates (not ciphertext byte offsets).
#[tokio::test]
async fn test_edit_list_offset_is_plaintext_not_ciphertext() {
    // 3 chunks (3 * 65536) + remainder.
    let payload_len: usize = 200_000;
    let plaintext: Vec<u8> = (0..payload_len).map(|i| (i % 251) as u8).collect();

    let skpk = crypt4gh::keys::generate_private_key();
    let (sk, pk) = skpk.split_at(32);
    let pub_bytes = pk.to_vec();

    let mut recipients = HashSet::new();
    recipients.insert(recipient_keys_from_pubkey(&pub_bytes));

    // Decrypt key for the receiver (the decryptor privkey is the first 32 bytes).
    let dec_key = C4ghKeys {
        method: 0,
        privkey: sk.to_vec(),
        recipient_pubkey: pub_bytes.clone(),
    };

    // 1) Encrypt the full plaintext without a range (no edit-list yet).
    let enc_ciphertext = {
        let mut reader = std::io::Cursor::new(&plaintext);
        let mut writer: Vec<u8> = Vec::new();
        crypt4gh::encrypt(&recipients, &mut reader, &mut writer, 0, None)
            .expect("crypt4gh encrypt failed");
        writer
    };

    // 2) Rearrange for a plaintext slice: offset=65536, length=65536 (second chunk).
    //    This creates the header edit-list packet used by random-access decrypt.
    let rearranged_ciphertext = {
        let mut reader = std::io::Cursor::new(&enc_ciphertext);
        let mut writer: Vec<u8> = Vec::new();
        crypt4gh::rearrange(
            vec![dec_key.clone()],
            &mut reader,
            &mut writer,
            65_536,
            Some(65_536),
        )
        .expect("crypt4gh rearrange failed");
        writer
    };

    // 3) Decrypt using the edit-list inside the header and verify correct plaintext bytes.
    let got_plaintext = {
        let mut reader = std::io::Cursor::new(&rearranged_ciphertext);
        let mut writer: Vec<u8> = Vec::new();
        crypt4gh::decrypt(
            &[dec_key],
            &mut reader,
            &mut writer,
            0,
            None,
            &None,
        )
        .expect("crypt4gh decrypt failed");
        writer
    };

    let expected = plaintext[65_536..(65_536 + 65_536)].to_vec();
    assert_eq!(
        got_plaintext, expected,
        "edit-list offset must use plaintext coordinates"
    );
}

