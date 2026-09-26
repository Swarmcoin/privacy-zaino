//! Fixed test vectors for transactions.

use arbitrary::v5_transactions;
use chrono::DateTime;
use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use rand::{seq::IteratorRandom, thread_rng};

use crate::{
    block::{Block, Height, MAX_BLOCK_BYTES},
    parameters::Network,
    primitives::zcash_primitives::PrecomputedTxData,
    serialization::{SerializationError, ZcashDeserialize, ZcashDeserializeInto, ZcashSerialize},
    transaction::{sighash::SigHasher, txid::TxIdBuilder},
    transparent::Script,
};

use zebra_test::{
    vectors::{ZIP143_1, ZIP143_2, ZIP243_1, ZIP243_2, ZIP243_3},
    zip0143, zip0243, zip0244,
};

use super::super::*;

lazy_static! {
    pub static ref EMPTY_V5_TX: Transaction = Transaction::V5 {
        network_upgrade: NetworkUpgrade::Nu5,
        lock_time: LockTime::min_lock_time_timestamp(),
        expiry_height: block::Height(0),
        inputs: Vec::new(),
        outputs: Vec::new(),
        sapling_shielded_data: None,
        orchard_shielded_data: None,
    };
}

/// Build a mock output list for pre-V5 transactions, with (index+1)
/// copies of `output`, which is used to computed the sighash.
///
/// Pre-V5, the entire output list is not used; only the output for the
/// given index is read. Therefore, we just need a list where `array[index]`
/// is the given `output`.
fn mock_pre_v5_output_list(output: transparent::Output, index: usize) -> Vec<transparent::Output> {
    std::iter::repeat_n(output, index + 1).collect()
}

#[test]
fn transactionhash_struct_from_str_roundtrip() {
    let _init_guard = zebra_test::init();

    let hash: Hash = "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf"
        .parse()
        .unwrap();

    assert_eq!(
        format!("{hash:?}"),
        r#"transaction::Hash("3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf")"#
    );
    assert_eq!(
        hash.to_string(),
        "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf"
    );
}

#[test]
fn auth_digest_struct_from_str_roundtrip() {
    let _init_guard = zebra_test::init();

    let digest: AuthDigest = "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf"
        .parse()
        .unwrap();

    assert_eq!(
        format!("{digest:?}"),
        r#"AuthDigest("3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf")"#
    );
    assert_eq!(
        digest.to_string(),
        "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf"
    );
}

#[test]
fn wtx_id_struct_from_str_roundtrip() {
    let _init_guard = zebra_test::init();

    let wtx_id: WtxId = "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .unwrap();

    assert_eq!(
        format!("{wtx_id:?}"),
        r#"WtxId { id: transaction::Hash("3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf"), auth_digest: AuthDigest("0000000000000000000000000000000000000000000000000000000000000001") }"#
    );
    assert_eq!(
        wtx_id.to_string(),
        "3166411bd5343e0b284a108f39a929fbbb62619784f8c6dafe520703b5b446bf0000000000000000000000000000000000000000000000000000000000000001"
    );
}

#[test]
fn librustzcash_tx_deserialize_and_round_trip() {
    let _init_guard = zebra_test::init();

    let tx = Transaction::zcash_deserialize(&zebra_test::vectors::GENERIC_TESTNET_TX[..])
        .expect("transaction test vector from librustzcash should deserialize");

    let mut data2 = Vec::new();
    tx.zcash_serialize(&mut data2).expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::GENERIC_TESTNET_TX[..], &data2[..]);
}

#[test]
fn librustzcash_tx_hash() {
    let _init_guard = zebra_test::init();

    let tx = Transaction::zcash_deserialize(&zebra_test::vectors::GENERIC_TESTNET_TX[..])
        .expect("transaction test vector from librustzcash should deserialize");

    // TxID taken from comment in zebra_test::vectors
    let hash = tx.hash();
    let expected = "64f0bd7fe30ce23753358fe3a2dc835b8fba9c0274c4e2c54a6f73114cb55639"
        .parse::<Hash>()
        .expect("hash should parse correctly");

    assert_eq!(hash, expected);
}

#[test]
fn doesnt_deserialize_transaction_with_invalid_value_balance() {
    let _init_guard = zebra_test::init();

    let dummy_transaction = Transaction::V4 {
        inputs: vec![],
        outputs: vec![],
        lock_time: LockTime::Height(Height(1)),
        expiry_height: Height(10),
        joinsplit_data: None,
        sapling_shielded_data: None,
    };

    let mut input_bytes = Vec::new();
    dummy_transaction
        .zcash_serialize(&mut input_bytes)
        .expect("dummy transaction should serialize");
    // Set value balance to non-zero
    // There are 4 * 4 byte fields and 2 * 1 byte compact sizes = 18 bytes before the 8 byte amount
    // (Zcash is little-endian unless otherwise specified:
    // https://zips.z.cash/protocol/nu5.pdf#endian)
    input_bytes[18] = 1;

    let result = Transaction::zcash_deserialize(&input_bytes[..]);

    assert!(matches!(
        result,
        Err(SerializationError::BadTransactionBalance)
    ));
}

#[test]
fn zip143_deserialize_and_round_trip() {
    let _init_guard = zebra_test::init();

    let tx1 = Transaction::zcash_deserialize(&zebra_test::vectors::ZIP143_1[..])
        .expect("transaction test vector from ZIP143 should deserialize");

    let mut data1 = Vec::new();
    tx1.zcash_serialize(&mut data1)
        .expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::ZIP143_1[..], &data1[..]);

    let tx2 = Transaction::zcash_deserialize(&zebra_test::vectors::ZIP143_2[..])
        .expect("transaction test vector from ZIP143 should deserialize");

    let mut data2 = Vec::new();
    tx2.zcash_serialize(&mut data2)
        .expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::ZIP143_2[..], &data2[..]);
}

#[test]
fn zip243_deserialize_and_round_trip() {
    let _init_guard = zebra_test::init();

    let tx1 = Transaction::zcash_deserialize(&zebra_test::vectors::ZIP243_1[..])
        .expect("transaction test vector from ZIP243 should deserialize");

    let mut data1 = Vec::new();
    tx1.zcash_serialize(&mut data1)
        .expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::ZIP243_1[..], &data1[..]);

    let tx2 = Transaction::zcash_deserialize(&zebra_test::vectors::ZIP243_2[..])
        .expect("transaction test vector from ZIP243 should deserialize");

    let mut data2 = Vec::new();
    tx2.zcash_serialize(&mut data2)
        .expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::ZIP243_2[..], &data2[..]);

    let tx3 = Transaction::zcash_deserialize(&zebra_test::vectors::ZIP243_3[..])
        .expect("transaction test vector from ZIP243 should deserialize");

    let mut data3 = Vec::new();
    tx3.zcash_serialize(&mut data3)
        .expect("tx should serialize");

    assert_eq!(&zebra_test::vectors::ZIP243_3[..], &data3[..]);
}

#[test]
fn deserialize_large_transaction() {
    let _init_guard = zebra_test::init();

    // Create a dummy input and output.
    let input =
        transparent::Input::zcash_deserialize(&zebra_test::vectors::DUMMY_INPUT1[..]).unwrap();
    let output =
        transparent::Output::zcash_deserialize(&zebra_test::vectors::DUMMY_OUTPUT1[..]).unwrap();

    // Serialize the input so that we can determine its serialized size.
    let mut input_data = Vec::new();
    input
        .zcash_serialize(&mut input_data)
        .expect("input should serialize");

    // Calculate the number of inputs that fit into the transaction size limit.
    let tx_inputs_num = MAX_BLOCK_BYTES as usize / input_data.len();

    // Set the precalculated amount of inputs and a single output.
    let inputs = std::iter::repeat_n(input, tx_inputs_num).collect::<Vec<_>>();

    // Create an oversized transaction. Adding the output and lock time causes
    // the transaction to overflow the threshold.
    let oversized_tx = Transaction::V1 {
        inputs,
        outputs: vec![output],
        lock_time: LockTime::Time(DateTime::from_timestamp(61, 0).unwrap()),
    };

    // Serialize the transaction.
    let mut tx_data = Vec::new();
    oversized_tx
        .zcash_serialize(&mut tx_data)
        .expect("transaction should serialize");

    // Check that the transaction is oversized.
    assert!(tx_data.len() > MAX_BLOCK_BYTES as usize);

    // The deserialization should fail because the transaction is too big.
    Transaction::zcash_deserialize(&tx_data[..])
        .expect_err("transaction should not deserialize due to its size");
}

// Transaction V5 test vectors

/// An empty transaction v5, with no Orchard, Sapling, or Transparent data
///
/// empty transaction are invalid, but Zebra only checks this rule in
/// zebra_consensus::transaction::check::has_inputs_and_outputs
#[test]
fn empty_v5_round_trip() {
    let _init_guard = zebra_test::init();

    let tx: &Transaction = &EMPTY_V5_TX;

    let data = tx.zcash_serialize_to_vec().expect("tx should serialize");
    let tx2: &Transaction = &data
        .zcash_deserialize_into()
        .expect("tx should deserialize");

    assert_eq!(tx, tx2);

    let data2 = tx2
        .zcash_serialize_to_vec()
        .expect("vec serialization is infallible");

    assert_eq!(data, data2, "data must be equal if structs are equal");
}

/// An empty transaction v4, with no Sapling, Sprout, or Transparent data
///
/// empty transaction are invalid, but Zebra only checks this rule in
/// zebra_consensus::transaction::check::has_inputs_and_outputs
#[test]
fn empty_v4_round_trip() {
    let _init_guard = zebra_test::init();

    let tx = Transaction::V4 {
        inputs: Vec::new(),
        outputs: Vec::new(),
        lock_time: LockTime::min_lock_time_timestamp(),
        expiry_height: block::Height(0),
        joinsplit_data: None,
        sapling_shielded_data: None,
    };

    let data = tx.zcash_serialize_to_vec().expect("tx should serialize");
    let tx2 = data
        .zcash_deserialize_into()
        .expect("tx should deserialize");

    assert_eq!(tx, tx2);

    let data2 = tx2
        .zcash_serialize_to_vec()
        .expect("vec serialization is infallible");

    assert_eq!(data, data2, "data must be equal if structs are equal");
}

/// Check if an empty V5 transaction can be deserialized by librustzcash too.
#[test]
fn empty_v5_librustzcash_round_trip() {
    let _init_guard = zebra_test::init();

    let tx: &Transaction = &EMPTY_V5_TX;
    let nu = tx.network_upgrade().expect("network upgrade");

    tx.to_librustzcash(nu).expect(
        "librustzcash deserialization might work for empty zebra serialized transactions. \
        Hint: if empty transactions fail, but other transactions work, delete this test",
    );
}

#[test]
fn invalid_orchard_nullifier() {
    let _init_guard = zebra_test::init();

    use std::convert::TryFrom;

    // generated by proptest using something as:
    // ```rust
    // ...
    // array::uniform32(any::<u8>()).prop_map(|x| Self::try_from(x).unwrap()).boxed()
    // ...
    // ```
    let invalid_nullifier_bytes = [
        62, 157, 27, 63, 100, 228, 1, 82, 140, 16, 238, 78, 68, 19, 221, 184, 189, 207, 230, 95,
        194, 216, 165, 24, 110, 221, 139, 195, 106, 98, 192, 71,
    ];

    assert_eq!(
        orchard::Nullifier::try_from(invalid_nullifier_bytes)
            .err()
            .unwrap()
            .to_string(),
        SerializationError::Parse("Invalid pallas::Base value for orchard Nullifier").to_string()
    );
}

/// Do a round-trip test via librustzcash on fake v5 transactions created from v4 transactions
/// in the block test vectors.
/// Makes sure that zebra-serialized transactions can be deserialized by librustzcash.
#[test]
fn fake_v5_librustzcash_round_trip() {
    let _init_guard = zebra_test::init();
    for network in Network::iter() {
        fake_v5_librustzcash_round_trip_for_network(network);
    }
}

fn fake_v5_librustzcash_round_trip_for_network(network: Network) {
    let block_iter = network.block_iter();

    let overwinter_activation_height = NetworkUpgrade::Overwinter
        .activation_height(&network)
        .expect("a valid height")
        .0;

    let nu5_activation_height = NetworkUpgrade::Nu5
        .activation_height(&network)
        .unwrap_or(Height::MAX_EXPIRY_HEIGHT)
        .0;

    // skip blocks that are before overwinter as they will not have a valid consensus branch id
    // skip blocks equal or greater Nu5 activation as they are already v5 transactions
    let blocks_after_overwinter_and_before_nu5 = block_iter
        .skip_while(|(height, _)| **height < overwinter_activation_height)
        .take_while(|(height, _)| **height < nu5_activation_height);

    for (height, original_bytes) in blocks_after_overwinter_and_before_nu5 {
        let original_block = original_bytes
            .zcash_deserialize_into::<Block>()
            .expect("block is structurally valid");

        let mut fake_block = original_block.clone();
        fake_block.transactions = fake_block
            .transactions
            .iter()
            .map(AsRef::as_ref)
            .map(|t| arbitrary::transaction_to_fake_v5(t, &network, Height(*height)))
            .map(Into::into)
            .collect();

        // test each transaction
        for (original_tx, fake_tx) in original_block
            .transactions
            .iter()
            .zip(fake_block.transactions.iter())
        {
            assert_ne!(
                &original_tx, &fake_tx,
                "v1-v4 transactions must change when converted to fake v5"
            );

            let fake_bytes = fake_tx
                .zcash_serialize_to_vec()
                .expect("vec serialization is infallible");

            assert_ne!(
                &original_bytes[..],
                fake_bytes,
                "v1-v4 transaction data must change when converted to fake v5"
            );

            let nu = fake_tx.network_upgrade().expect("network upgrade");

            fake_tx
                .to_librustzcash(nu)
                .expect("librustzcash deserialization must work for zebra serialized transactions");
        }
    }
}

#[test]
fn zip244_round_trip() -> Result<()> {
    let _init_guard = zebra_test::init();

    for test in zip0244::TEST_VECTORS.iter() {
        let tx = test.tx.zcash_deserialize_into::<Transaction>()?;
        let reencoded = tx.zcash_serialize_to_vec()?;

        assert_eq!(test.tx, reencoded);

        let nu = tx.network_upgrade().expect("network upgrade");

        tx.to_librustzcash(nu)
            .expect("librustzcash deserialization must work for zebra serialized transactions");
    }

    Ok(())
}

#[test]
fn zip244_txid() -> Result<()> {
    let _init_guard = zebra_test::init();

    for test in zip0244::TEST_VECTORS.iter() {
        let txid = TxIdBuilder::new(&test.tx.zcash_deserialize_into::<Transaction>()?)
            .txid()
            .expect("txid");

        assert_eq!(txid.0, test.txid);
    }

    Ok(())
}

#[test]
fn zip244_auth_digest() -> Result<()> {
    let _init_guard = zebra_test::init();

    for test in zip0244::TEST_VECTORS.iter() {
        let transaction = test.tx.zcash_deserialize_into::<Transaction>()?;
        let auth_digest = transaction.auth_digest();
        assert_eq!(
            auth_digest
                .expect("must have auth_digest since it must be a V5 transaction")
                .0,
            test.auth_digest
        );
    }

    Ok(())
}

#[test]
fn test_vec143_1() -> Result<()> {
    let _init_guard = zebra_test::init();

    let transaction = ZIP143_1.zcash_deserialize_into::<Transaction>()?;

    let hasher = SigHasher::new(
        &transaction,
        NetworkUpgrade::Overwinter,
        Arc::new(Vec::new()),
    )
    .expect("network upgrade is valid for tx");

    let hash = hasher.sighash(HashType::ALL, None);
    let expected = "a1f1a4e5cd9bd522322d661edd2af1bf2a7019cfab94ece18f4ba935b0a19073";
    let result = hex::encode(hash);
    let span = tracing::span!(
        tracing::Level::ERROR,
        "compare_final",
        expected.len = expected.len(),
        buf.len = result.len()
    );
    let _guard = span.enter();
    assert_eq!(expected, result);

    Ok(())
}

#[test]
fn test_vec143_2() -> Result<()> {
    let _init_guard = zebra_test::init();

    let transaction = ZIP143_2.zcash_deserialize_into::<Transaction>()?;

    let value = hex::decode("2f6e04963b4c0100")?.zcash_deserialize_into::<Amount<_>>()?;
    let lock_script = Script::new(&hex::decode("53")?);
    let input_ind = 1;
    let output = transparent::Output {
        value,
        lock_script: lock_script.clone(),
    };
    let all_previous_outputs = mock_pre_v5_output_list(output, input_ind);

    let hasher = SigHasher::new(
        &transaction,
        NetworkUpgrade::Overwinter,
        Arc::new(all_previous_outputs),
    )
    .expect("network upgrade is valid for tx");

    let hash = hasher.sighash(
        HashType::SINGLE,
        Some((input_ind, lock_script.as_raw_bytes().to_vec())),
    );
    let expected = "23652e76cb13b85a0e3363bb5fca061fa791c40c533eccee899364e6e60bb4f7";
    let result: &[u8] = hash.as_ref();
    let result = hex::encode(result);
    let span = tracing::span!(
        tracing::Level::ERROR,
        "compare_final",
        expected.len = expected.len(),
        buf.len = result.len()
    );
    let _guard = span.enter();
    assert_eq!(expected, result);

    Ok(())
}

#[test]
fn test_vec243_1() -> Result<()> {
    let _init_guard = zebra_test::init();

    let transaction = ZIP243_1.zcash_deserialize_into::<Transaction>()?;

    let hasher = SigHasher::new(&transaction, NetworkUpgrade::Sapling, Arc::new(Vec::new()))
        .expect("network upgrade is valid for tx");

    let hash = hasher.sighash(HashType::ALL, None);
    let expected = "63d18534de5f2d1c9e169b73f9c783718adbef5c8a7d55b5e7a37affa1dd3ff3";
    let result = hex::encode(hash);
    let span = tracing::span!(
        tracing::Level::ERROR,
        "compare_final",
        expected.len = expected.len(),
        buf.len = result.len()
    );
    let _guard = span.enter();
    assert_eq!(expected, result);

    let precomputed_tx_data =
        PrecomputedTxData::new(&transaction, NetworkUpgrade::Sapling, Arc::new(Vec::new()))
            .expect("network upgrade is valid for tx");
    let alt_sighash =
        crate::primitives::zcash_primitives::sighash(&precomputed_tx_data, HashType::ALL, None);
    let result = hex::encode(alt_sighash);
    assert_eq!(expected, result);

    Ok(())
}

#[test]
fn test_vec243_2() -> Result<()> {
    let _init_guard = zebra_test::init();

    let transaction = ZIP243_2.zcash_deserialize_into::<Transaction>()?;

    let value = hex::decode("adedf02996510200")?.zcash_deserialize_into::<Amount<_>>()?;
    let lock_script = Script::new(&[]);
    let input_ind = 1;
    let output = transparent::Output {
        value,
        lock_script: lock_script.clone(),
    };
    let all_previous_outputs = mock_pre_v5_output_list(output, input_ind);

    let hasher = SigHasher::new(
        &transaction,
        NetworkUpgrade::Sapling,
        Arc::new(all_previous_outputs),
    )
    .expect("network upgrade is valid for tx");

    let hash = hasher.sighash(
        HashType::NONE,
        Some((input_ind, lock_script.as_raw_bytes().to_vec())),
    );
    let expected = "bbe6d84f57c56b29b914c694baaccb891297e961de3eb46c68e3c89c47b1a1db";
    let result = hex::encode(hash);
    let span = tracing::span!(
        tracing::Level::ERROR,
        "compare_final",
        expected.len = expected.len(),
        buf.len = result.len()
    );
    let _guard = span.enter();
    assert_eq!(expected, result);

    let lock_script = Script::new(&[]);
    let prevout = transparent::Output {
        value,
        lock_script: lock_script.clone(),
    };
    let index = input_ind;
    let all_previous_outputs = mock_pre_v5_output_list(prevout, input_ind);

    let precomputed_tx_data = PrecomputedTxData::new(
        &transaction,
        NetworkUpgrade::Sapling,
        Arc::new(all_previous_outputs),
    )
    .expect("network upgrade is valid for tx");
    let alt_sighash = crate::primitives::zcash_primitives::sighash(
        &precomputed_tx_data,
        HashType::NONE,
        Some((index, lock_script.as_raw_bytes().to_vec())),
    );
    let result = hex::encode(alt_sighash);
    assert_eq!(expected, result);

    Ok(())
}

#[test]
fn test_vec243_3() -> Result<()> {
    let _init_guard = zebra_test::init();

    let transaction = ZIP243_3.zcash_deserialize_into::<Transaction>()?;

    let value = hex::decode("80f0fa0200000000")?.zcash_deserialize_into::<Amount<_>>()?;
    let lock_script = Script::new(&hex::decode(
        "76a914507173527b4c3318a2aecd793bf1cfed705950cf88ac",
    )?);
    let input_ind = 0;
    let all_previous_outputs = vec![transparent::Output {
        value,
        lock_script: lock_script.clone(),
    }];

    let hasher = SigHasher::new(
        &transaction,
        NetworkUpgrade::Sapling,
        Arc::new(all_previous_outputs),
    )
    .expect("network upgrade is valid for tx");

    let hash = hasher.sighash(
        HashType::ALL,
        Some((input_ind, lock_script.as_raw_bytes().to_vec())),
    );
    let expected = "f3148f80dfab5e573d5edfe7a850f5fd39234f80b5429d3a57edcc11e34c585b";
    let result = hex::encode(hash);
    let span = tracing::span!(
        tracing::Level::ERROR,
        "compare_final",
        expected.len = expected.len(),
        buf.len = result.len()
    );
    let _guard = span.enter();
    assert_eq!(expected, result);

    let lock_script = Script::new(&hex::decode(
        "76a914507173527b4c3318a2aecd793bf1cfed705950cf88ac",
    )?);
    let prevout = transparent::Output {
        value,
        lock_script: lock_script.clone(),
    };
    let index = input_ind;

    let all_previous_outputs = vec![prevout];
    let precomputed_tx_data = PrecomputedTxData::new(
        &transaction,
        NetworkUpgrade::Sapling,
        Arc::new(all_previous_outputs),
    )
    .expect("network upgrade is valid for tx");
    let alt_sighash = crate::primitives::zcash_primitives::sighash(
        &precomputed_tx_data,
        HashType::ALL,
        Some((index, lock_script.as_raw_bytes().to_vec())),
    );
    let result = hex::encode(alt_sighash);
    assert_eq!(expected, result);

    Ok(())
}

#[test]
fn zip143_sighash() -> Result<()> {
    let _init_guard = zebra_test::init();

    for (i, test) in zip0143::TEST_VECTORS.iter().enumerate() {
        let transaction = test.tx.zcash_deserialize_into::<Transaction>()?;
        let (input_index, output) = match test.transparent_input {
            Some(transparent_input) => (
                Some(transparent_input as usize),
                Some(transparent::Output {
                    value: test.amount.try_into()?,
                    lock_script: transparent::Script::new(test.script_code.as_ref()),
                }),
            ),
            None => (None, None),
        };
        let all_previous_outputs: Vec<_> = match output.clone() {
            Some(output) => mock_pre_v5_output_list(output, input_index.unwrap()),
            None => vec![],
        };
        let result = hex::encode(
            transaction
                .sighash(
                    NetworkUpgrade::try_from(test.consensus_branch_id).expect("network upgrade"),
                    HashType::from_bits(test.hash_type).expect("must be a valid HashType"),
                    Arc::new(all_previous_outputs),
                    input_index.map(|input_index| {
                        (
                            input_index,
                            output.unwrap().lock_script.as_raw_bytes().to_vec(),
                        )
                    }),
                )
                .expect("network upgrade is valid for tx"),
        );
        let expected = hex::encode(test.sighash);
        assert_eq!(expected, result, "test #{i}: sighash does not match");
    }

    Ok(())
}

#[test]
fn zip243_sighash() -> Result<()> {
    let _init_guard = zebra_test::init();

    for (i, test) in zip0243::TEST_VECTORS.iter().enumerate() {
        let transaction = test.tx.zcash_deserialize_into::<Transaction>()?;
        let (input_index, output) = match test.transparent_input {
            Some(transparent_input) => (
                Some(transparent_input as usize),
                Some(transparent::Output {
                    value: test.amount.try_into()?,
                    lock_script: transparent::Script::new(test.script_code.as_ref()),
                }),
            ),
            None => (None, None),
        };
        let all_previous_outputs: Vec<_> = match output.clone() {
            Some(output) => mock_pre_v5_output_list(output, input_index.unwrap()),
            None => vec![],
        };
        let result = hex::encode(
            transaction
                .sighash(
                    NetworkUpgrade::try_from(test.consensus_branch_id).expect("network upgrade"),
                    HashType::from_bits(test.hash_type).expect("must be a valid HashType"),
                    Arc::new(all_previous_outputs),
                    input_index.map(|input_index| {
                        (
                            input_index,
                            output.unwrap().lock_script.as_raw_bytes().to_vec(),
                        )
                    }),
                )
                .expect("network upgrade is valid for tx"),
        );
        let expected = hex::encode(test.sighash);
        assert_eq!(expected, result, "test #{i}: sighash does not match");
    }

    Ok(())
}

#[test]
fn zip244_sighash() -> Result<()> {
    let _init_guard = zebra_test::init();

    for (i, test) in zip0244::TEST_VECTORS.iter().enumerate() {
        let transaction = test.tx.zcash_deserialize_into::<Transaction>()?;

        let all_previous_outputs: Arc<Vec<_>> = Arc::new(
            test.amounts
                .iter()
                .zip(test.script_pubkeys.iter())
                .map(|(amount, script_pubkey)| transparent::Output {
                    value: (*amount).try_into().unwrap(),
                    lock_script: transparent::Script::new(script_pubkey.as_ref()),
                })
                .collect(),
        );

        let result = hex::encode(
            transaction
                .sighash(
                    NetworkUpgrade::Nu5,
                    HashType::ALL,
                    all_previous_outputs.clone(),
                    None,
                )
                .expect("network upgrade is valid for tx"),
        );
        let expected = hex::encode(test.sighash_shielded);
        assert_eq!(expected, result, "test #{i}: sighash does not match");

        if let Some(sighash_all) = test.sighash_all {
            let result = hex::encode(
                transaction
                    .sighash(
                        NetworkUpgrade::Nu5,
                        HashType::ALL,
                        all_previous_outputs,
                        test.transparent_input
                            .map(|idx| (idx as _, test.script_pubkeys[idx as usize].clone())),
                    )
                    .expect("network upgrade is valid for tx"),
            );
            let expected = hex::encode(sighash_all);
            assert_eq!(expected, result, "test #{i}: sighash does not match");
        }
    }

    Ok(())
}

/// Real Orchard proofs from mined transactions must have the canonical size, and padding
/// a proof with trailing bytes must make it non-canonical (GHSA-jfw5-j458-pfv6). This
/// also cross-checks `expected_proof_size` against real proofs produced by the chain.
#[test]
fn orchard_proof_size_is_canonical() {
    let mut checked = 0;

    for net in Network::iter() {
        for tx in v5_transactions(net.block_iter()) {
            let Some(shielded_data) = tx.orchard_shielded_data() else {
                continue;
            };

            // A real, mined Orchard proof has the canonical length for its actions.
            assert!(
                shielded_data.proof_size_is_canonical(),
                "a real Orchard proof should be canonically sized"
            );

            // Padding the proof with trailing data must break canonicity.
            let mut padded = shielded_data.clone();
            padded.proof.0.push(0);
            assert!(
                !padded.proof_size_is_canonical(),
                "a padded Orchard proof must not be considered canonical"
            );

            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "expected at least one Orchard transaction in the test vectors"
    );
}

#[test]
fn consensus_branch_id() {
    for net in Network::iter() {
        for tx in v5_transactions(net.block_iter()).filter(|tx| {
            !tx.has_transparent_inputs() && tx.has_shielded_data() && tx.network_upgrade().is_some()
        }) {
            let tx_nu = tx
                .network_upgrade()
                .expect("this test shouldn't use txs without a network upgrade");

            let any_other_nu = NetworkUpgrade::iter()
                .filter(|&nu| nu != tx_nu)
                .choose(&mut thread_rng())
                .expect("there must be a network upgrade other than the tx one");

            // All computations should succeed under the tx nu.

            tx.to_librustzcash(tx_nu)
                .expect("tx is convertible under tx nu");
            PrecomputedTxData::new(&tx, tx_nu, Arc::new(Vec::new()))
                .expect("network upgrade is valid for tx");
            sighash::SigHasher::new(&tx, tx_nu, Arc::new(Vec::new()))
                .expect("network upgrade is valid for tx");
            tx.sighash(tx_nu, HashType::ALL, Arc::new(Vec::new()), None)
                .expect("network upgrade is valid for tx");

            // All computations should fail under an nu other than the tx one.

            tx.to_librustzcash(any_other_nu)
                .expect_err("tx is not convertible under nu other than the tx one");

            let err = PrecomputedTxData::new(&tx, any_other_nu, Arc::new(Vec::new())).unwrap_err();
            assert!(
                matches!(err, crate::Error::InvalidConsensusBranchId),
                "precomputing tx sighash data errors under nu other than the tx one"
            );

            let err = sighash::SigHasher::new(&tx, any_other_nu, Arc::new(Vec::new())).unwrap_err();
            assert!(
                matches!(err, crate::Error::InvalidConsensusBranchId),
                "creating the sighasher errors under nu other than the tx one"
            );

            let err = tx
                .sighash(any_other_nu, HashType::ALL, Arc::new(Vec::new()), None)
                .unwrap_err();
            assert!(
                matches!(err, crate::Error::InvalidConsensusBranchId),
                "the sighash computation errors under nu other than the tx one"
            );
        }
    }
}

#[test]
fn binding_signatures() {
    let _init_guard = zebra_test::init();

    for net in Network::iter() {
        let sapling_activation_height = NetworkUpgrade::Sapling
            .activation_height(&net)
            .expect("a valid height")
            .0;

        let mut at_least_one_v4_checked = false;
        let mut at_least_one_v5_checked = false;

        for (height, block) in net
            .block_iter()
            .skip_while(|(height, _)| **height < sapling_activation_height)
        {
            let nu = NetworkUpgrade::current(&net, Height(*height));

            for tx in block
                .zcash_deserialize_into::<Block>()
                .expect("a valid block")
                .transactions
            {
                match &*tx {
                    Transaction::V1 { .. } | Transaction::V2 { .. } | Transaction::V3 { .. } => (),
                    Transaction::V4 {
                        sapling_shielded_data,
                        ..
                    } => {
                        if let Some(sapling_shielded_data) = sapling_shielded_data {
                            let sighash = tx
                                .sighash(nu, HashType::ALL, Arc::new(Vec::new()), None)
                                .expect("network upgrade is valid for tx");

                            let bvk = redjubjub::VerificationKey::try_from(
                                sapling_shielded_data.binding_verification_key(),
                            )
                            .expect("a valid redjubjub::VerificationKey");

                            bvk.verify(sighash.as_ref(), &sapling_shielded_data.binding_sig)
                                .expect("must pass verification");

                            at_least_one_v4_checked = true;
                        }
                    }
                    Transaction::V5 {
                        sapling_shielded_data,
                        ..
                    } => {
                        if let Some(sapling_shielded_data) = sapling_shielded_data {
                            // V5 txs have the outputs spent by their transparent inputs hashed into
                            // their SIGHASH, so we need to exclude txs with transparent inputs.
                            //
                            // References:
                            //
                            // <https://zips.z.cash/zip-0244#s-2c-amounts-sig-digest>
                            // <https://zips.z.cash/zip-0244#s-2d-scriptpubkeys-sig-digest>
                            if tx.has_transparent_inputs() {
                                continue;
                            }

                            let sighash = tx
                                .sighash(nu, HashType::ALL, Arc::new(Vec::new()), None)
                                .expect("network upgrade is valid for tx");

                            let bvk = redjubjub::VerificationKey::try_from(
                                sapling_shielded_data.binding_verification_key(),
                            )
                            .expect("a valid redjubjub::VerificationKey");

                            bvk.verify(sighash.as_ref(), &sapling_shielded_data.binding_sig)
                                .expect("verification passes");

                            at_least_one_v5_checked = true;
                        }
                    }
                    Transaction::V6 {
                        sapling_shielded_data,
                        ..
                    } => {
                        if let Some(sapling_shielded_data) = sapling_shielded_data {
                            // V6 txs have the outputs spent by their transparent inputs hashed into
                            // their SIGHASH, so we need to exclude txs with transparent inputs.
                            //
                            // References:
                            //
                            // <https://zips.z.cash/zip-0244#s-2c-amounts-sig-digest>
                            // <https://zips.z.cash/zip-0244#s-2d-scriptpubkeys-sig-digest>
                            if tx.has_transparent_inputs() {
                                continue;
                            }

                            let sighash = tx
                                .sighash(nu, HashType::ALL, Arc::new(Vec::new()), None)
                                .expect("network upgrade is valid for tx");

                            let bvk = redjubjub::VerificationKey::try_from(
                                sapling_shielded_data.binding_verification_key(),
                            )
                            .expect("a valid redjubjub::VerificationKey");

                            bvk.verify(sighash.as_ref(), &sapling_shielded_data.binding_sig)
                                .expect("verification passes");

                            at_least_one_v5_checked = true;
                        }
                    }
                }
            }
        }

        assert!(at_least_one_v4_checked);
        assert!(at_least_one_v5_checked);
    }
}

/// Check that a v6 (Ironwood / NU6.3) transaction computes a txid and round-trips through
/// serialization.
///
/// Computing the txid drives [`Transaction::to_librustzcash`] into the librustzcash Ironwood fork's
/// v6 (ZIP-244) digest path, which is the runtime path that does not work against released
/// librustzcash. The branch id resolves to the fork's `BranchId::Nu6_3`.
#[test]
fn v6_ironwood_txid_and_roundtrip() {
    let _init_guard = zebra_test::init();

    let tx = Transaction::V6 {
        network_upgrade: NetworkUpgrade::Nu6_3,
        lock_time: LockTime::min_lock_time_timestamp(),
        expiry_height: block::Height(0),
        inputs: Vec::new(),
        outputs: Vec::new(),
        sapling_shielded_data: None,
        orchard_shielded_data: None,
        ironwood_shielded_data: None,
    };

    // Drives the librustzcash Ironwood fork's v6 digest computation.
    let txid = tx.hash();

    // The v6 wire format round-trips through Zebra's own (de)serializer.
    let bytes = tx
        .zcash_serialize_to_vec()
        .expect("v6 transaction serializes");
    let tx2: Transaction = bytes
        .zcash_deserialize_into()
        .expect("v6 transaction deserializes");

    assert_eq!(tx, tx2);
    assert_eq!(tx2.hash(), txid, "txid is stable across serialization");
}

/// A v6 transaction carrying populated Orchard-v6 and Ironwood bundles round-trips through Zebra's
/// own v6 (de)serializer.
///
/// Unlike [`v6_ironwood_txid_and_roundtrip`], this does not compute a txid: that drives the
/// librustzcash fork's parser, which does not accept the structurally-fake proof bytes the test
/// helpers produce. This exercises Zebra's self-contained v6 wire codec for *non-empty* bundles
/// (the empty-bundle path is already covered above).
#[test]
fn v6_transaction_with_bundles_round_trips() {
    use crate::ironwood;
    use crate::orchard::{Flags, ShieldedDataV6};

    let _init_guard = zebra_test::init();
    let zero = Amount::try_from(0).expect("zero is a valid amount");

    let orchard = ShieldedDataV6::new(arbitrary::fake_v6_orchard_shielded_data(
        Flags::ENABLE_SPENDS | Flags::ENABLE_OUTPUTS,
        zero,
        1,
    ));
    let ironwood = ironwood::ShieldedData::new(ShieldedDataV6::new(
        arbitrary::fake_v6_orchard_shielded_data(Flags::ENABLE_SPENDS, zero, 2),
    ));

    let tx = arbitrary::fake_v6_transaction(NetworkUpgrade::Nu6_3, Some(orchard), Some(ironwood));

    let bytes = tx
        .zcash_serialize_to_vec()
        .expect("v6 transaction serializes");
    let tx2: Transaction = bytes
        .zcash_deserialize_into()
        .expect("v6 transaction deserializes");
    assert_eq!(tx, tx2);
}

/// The `enableCrossAddress` flag (bit 2) is permitted only for the Ironwood pool: a v6 Orchard
/// bundle carrying it MUST be rejected at deserialization (matching `orchard::Flags::from_byte`,
/// which reserves bit 2 for `ValuePool::Orchard` in every tx version), while the same flag on the
/// Ironwood bundle round-trips.
///
/// The Orchard case is the wire-layer guard: without it, a crafted bundle deserializes and then
/// aborts the node in the txid-path `expect(...)` when `to_librustzcash` rejects the flag.
#[test]
fn v6_orchard_bundle_rejects_cross_address_flag_on_the_wire() {
    use crate::ironwood;
    use crate::orchard::{Flags, ShieldedDataV6};

    let _init_guard = zebra_test::init();
    let zero = Amount::try_from(0).expect("zero is a valid amount");

    // A v6 Orchard bundle with `enableCrossAddress` serializes (the flag byte is written as-is) but
    // MUST NOT deserialize.
    let orchard = ShieldedDataV6::new(arbitrary::fake_v6_orchard_shielded_data(
        Flags::ENABLE_SPENDS | Flags::ENABLE_CROSS_ADDRESS,
        zero,
        1,
    ));
    let tx = arbitrary::fake_v6_transaction(NetworkUpgrade::Nu6_3, Some(orchard), None);
    let bytes = tx
        .zcash_serialize_to_vec()
        .expect("v6 transaction serializes");
    let result: Result<Transaction, _> = bytes.zcash_deserialize_into();
    assert!(
        result.is_err(),
        "a v6 Orchard bundle with enableCrossAddress must be rejected on the wire",
    );

    // The same flag on the Ironwood bundle is valid and round-trips.
    let ironwood = ironwood::ShieldedData::new(ShieldedDataV6::new(
        arbitrary::fake_v6_orchard_shielded_data(
            Flags::ENABLE_SPENDS | Flags::ENABLE_CROSS_ADDRESS,
            zero,
            1,
        ),
    ));
    let tx = arbitrary::fake_v6_transaction(NetworkUpgrade::Nu6_3, None, Some(ironwood));
    let bytes = tx
        .zcash_serialize_to_vec()
        .expect("v6 transaction serializes");
    let tx2: Transaction = bytes
        .zcash_deserialize_into()
        .expect("a v6 Ironwood bundle with enableCrossAddress round-trips");
    assert_eq!(tx, tx2);
}

#[test]
fn test_coinbase_script() -> Result<()> {
    let _init_guard = zebra_test::init();

    let tx = hex::decode("0400008085202f89010000000000000000000000000000000000000000000000000000000000000000ffffffff0503b0e72100ffffffff04e8bbe60e000000001976a914ba92ff06081d5ff6542af8d3b2d209d29ba6337c88ac40787d010000000017a914931fec54c1fea86e574462cc32013f5400b891298738c94d010000000017a914c7a4285ed7aed78d8c0e28d7f1839ccb4046ab0c87286bee000000000017a914d45cb1adffb5215a42720532a076f02c7c778c908700000000b0e721000000000000000000000000").unwrap();

    let transaction = tx.zcash_deserialize_into::<Transaction>()?;

    let recoded_tx = transaction.zcash_serialize_to_vec().unwrap();
    assert_eq!(tx, recoded_tx);

    let data = transaction.inputs()[0].coinbase_script().unwrap();
    let expected = hex::decode("03b0e72100").unwrap();
    assert_eq!(data, expected);

    Ok(())
}

/// Regression test for the Orchard `rk` identity-point DoS vulnerability.
///
/// A v5 transaction whose Orchard action has `rk = [0u8; 32]` (the Pallas
/// identity point) **deserializes successfully** — Zebra performs no
/// identity-point check in [`crate::orchard::Action::zcash_deserialize`].
///
/// When the same transaction is subsequently fed to the Orchard Halo2 batch
/// verifier via [`orchard::bundle::BatchValidator::add_bundle`], the call
/// chain reaches `orchard::circuit::to_halo2_instance()`, which calls
/// `.coordinates().unwrap()` on the identity point.  `coordinates()` returns
/// `None` for the identity, so the `unwrap` **panics**, crashing the node.
///
/// ## Root cause
///
/// `zebra-chain/src/orchard/action.rs:83` reads `rk` as raw bytes with no
/// identity-point check: `reader.read_32_bytes()?.into()`.  The upstream
/// `orchard` crate defers validation to signature verification, but
/// `to_halo2_instance()` unwraps the coordinate extraction unconditionally.
///
/// An analogous identity check already exists for `ephemeral_key`
/// (`zebra-chain/src/orchard/keys.rs:225-238`), demonstrating the correct
/// pattern.
#[test]
fn orchard_rk_identity_point() {
    use group::prime::PrimeCurveAffine;
    use reddsa::Signature;

    use crate::{
        at_least_one,
        block::Height,
        orchard::{
            keys::EphemeralPublicKey, tree, Action, AuthorizedAction, EncryptedNote, Flags,
            NoteCommitment, Nullifier, ShieldedData, ValueCommitment, WrappedNoteKey,
        },
        primitives::Halo2Proof,
        serialization::ZcashSerialize,
    };
    use halo2::pasta::pallas;

    let _init_guard = zebra_test::init();

    // Construct an Orchard action with rk = [0u8; 32] (identity point).
    // Other fields use the Pallas generator or the identity as appropriate.
    let action = Action {
        // cv can be any valid Pallas point; identity is accepted here.
        cv: ValueCommitment(pallas::Affine::identity()),
        nullifier: Nullifier(pallas::Base::zero()),
        // rk = identity point — this is the vulnerability trigger.
        rk: [0u8; 32].into(),
        // cm_x is the x-coordinate of the note commitment.
        cm_x: NoteCommitment(pallas::Affine::identity()).extract_x(),
        // ephemeral_key must be non-identity; use the generator.
        ephemeral_key: EphemeralPublicKey(pallas::Affine::generator()),
        enc_ciphertext: EncryptedNote([0u8; 580]),
        out_ciphertext: WrappedNoteKey([0u8; 80]),
    };

    let shielded_data = ShieldedData {
        flags: Flags::ENABLE_SPENDS | Flags::ENABLE_OUTPUTS,
        value_balance: crate::amount::Amount::try_from(0).expect("zero is a valid amount"),
        shared_anchor: tree::Root::default(),
        // An empty proof is accepted at deserialization time.
        proof: Halo2Proof(vec![]),
        actions: at_least_one![AuthorizedAction {
            action,
            spend_auth_sig: Signature::from([0u8; 64]),
        }],
        binding_sig: Signature::from([0u8; 64]),
    };

    let tx = Transaction::V5 {
        network_upgrade: NetworkUpgrade::Nu5,
        lock_time: LockTime::unlocked(),
        expiry_height: Height(0),
        inputs: vec![],
        outputs: vec![],
        sapling_shielded_data: None,
        orchard_shielded_data: Some(shielded_data),
    };

    // Step 1: serialize the transaction.
    let tx_bytes = tx
        .zcash_serialize_to_vec()
        .expect("crafted transaction must serialize without error");

    // Step 2: deserialize
    Transaction::zcash_deserialize(&tx_bytes[..]).expect_err("rk = identity should fail");
}

/// Reproduction for GHSA-rgwx-8r98-p34c:
/// Coinbase Sapling spend vectors allocate before zero-spend consensus rule.
///
/// A V5 coinbase transaction with Sapling spends can be serialized and
/// deserialized — the parser allocates Sapling spend vectors (bounded by
/// `TrustedPreallocate::max_allocation()`) before any coinbase-specific
/// check. The consensus rule rejecting coinbase Sapling spends only runs
/// later in `zebra-consensus`, not during deserialization.
#[test]
fn coinbase_v5_with_sapling_spends_deserializes_successfully() {
    let _init_guard = zebra_test::init();

    let network = Network::Mainnet;

    // Find a real V4 transaction with Sapling spends from the test block vectors.
    let tx_with_spends = arbitrary::test_transactions(&network)
        .find(|(_, tx)| tx.sapling_spends_per_anchor().count() > 0);

    let Some((height, original_tx)) = tx_with_spends else {
        panic!("test block vectors must contain at least one transaction with Sapling spends");
    };

    let original_spend_count = original_tx.sapling_spends_per_anchor().count();
    assert!(
        original_spend_count > 0,
        "source transaction must have Sapling spends"
    );

    // Convert the V4 transaction to a fake V5 — this preserves valid Sapling data.
    let fake_v5 = arbitrary::transaction_to_fake_v5(&original_tx, &network, height);

    // Replace transparent inputs with a single coinbase input.
    let Transaction::V5 {
        lock_time,
        expiry_height,
        outputs,
        sapling_shielded_data,
        orchard_shielded_data,
        ..
    } = fake_v5
    else {
        panic!("transaction_to_fake_v5 must return V5");
    };

    // Confirm the fake V5 still has Sapling spends.
    let sapling_shielded_data =
        sapling_shielded_data.expect("converted V5 must retain Sapling shielded data with spends");

    let coinbase_tx = Transaction::V5 {
        network_upgrade: NetworkUpgrade::Nu5,
        lock_time,
        expiry_height,
        inputs: vec![transparent::Input::Coinbase {
            height,
            data: vec![0x00; 4],
            sequence: 0xFFFF_FFFF,
        }],
        outputs: if outputs.is_empty() {
            vec![transparent::Output {
                value: crate::amount::Amount::zero(),
                lock_script: Script::new(&[0u8; 20]),
            }]
        } else {
            outputs
        },
        sapling_shielded_data: Some(sapling_shielded_data),
        orchard_shielded_data,
    };

    // The constructed transaction must look like a coinbase with Sapling spends.
    assert!(coinbase_tx.is_coinbase(), "transaction must be coinbase");
    assert!(
        coinbase_tx.sapling_spends_per_anchor().count() > 0,
        "coinbase transaction has Sapling spends"
    );

    // Serialize it.
    let serialized = coinbase_tx
        .zcash_serialize_to_vec()
        .expect("coinbase V5 with Sapling spends must serialize");

    // Deserialize it — the parser must now reject coinbase transactions with
    // Sapling spends before allocating spend vectors (GHSA-rgwx-8r98-p34c fix).
    let err = serialized
        .zcash_deserialize_into::<Transaction>()
        .expect_err("coinbase with Sapling spends must be rejected during deserialization");

    assert!(
        err.to_string()
            .contains("coinbase transaction must not have Sapling spends"),
        "unexpected error: {err}"
    );
}

/// Two blocks of the SWARM production network exactly as the node serves them
/// (`getblock <height> 0`, read on 2026-09-26 from the chain whose genesis is
/// `01c34428b9e67cdd8345e0b365aaa37dd8d2d65d3869e0e5d77d567f2c39afdd`): block 1,
/// the first mined block, and block 94, the tip at the time. Each holds one
/// version 6 coinbase whose `nConsensusBranchId` is the SWARM production domain
/// `0x53574d31`. Before this domain was admitted, deserializing either failed
/// with "invalid consensus branch id" and the indexer could not read a block.
mod swarm_production {
    use super::*;

    const BLOCK_1_HEX: &str = "04000000ddaf392c7f567dd7e5e069385dd6d2d87da3aa65b3e04583dd7ce6b92844c301b580ef7277673296f194ebc4\
a072ba260bf649b15373c6037505b02be21ab0c22c403dd0b7e6055ac5248199f24a07e982aa0c042bd57c4c344ad303\
ac810bb5c5b9b76a51b80620000000000000000000000000000000000000000000000000000000000000001efd400500\
a53ccff66620ab857fc3924bbbfb8a0d02fe36ab07315a927a4d87e89217125f32d805d60c08ffc76404f3d639f39ee7\
1363e450f442d6618073041a9608248cd47098ef9bf5b957225e70e39c3d41683f3e1a0250b8210f471d6fb4cc105ae2\
7dede468403f3ab1281941c50c9df401bdba9803d869c21385061dfafe2bf73f6e2d3894b3f268d55dfeb04d1978481a\
bf7638194a5aa317351ef86ac4d87db0180e8f791c18b10574a02be7531ba3136ff35edce7174d2105b838e827fe8959\
e9cb82490e8c53c65d77403d9684eef2980b2a89510aa772f7bc1c329c4134ce53b4ad7efa3d117e1b83a713c669ac97\
b64497ec4cf9b0a5117441079b188c360a16b17279080a21ddf20390c45cf8690ff4fa29fd0f70152a0f581affe2e0f2\
bdc7f86a0b177671cd4a15d7b37ab6c66e8e39cce2423df2c372201279c5e2d97991d17a229aa7a7b9712e42f6dc4d00\
ff3058f395c6938444194820773b42726f175df81465393a7fc9819eed1bf5aac0500a7df15ade831313672557d011d0\
993435c4917c642e5df41e36bf6e20390c0cbe564379d48994c55fafc5fa9bf91dd65909e6f99f6897e8796193d0e4ac\
c948c653f8d7f8c815c8631e5385fd3cee9fa19af9ea121868f026df0f188fa779e351edd5c4ae3607b5cf2a0a2cfd9e\
24d51d5ec256e7d7e8af6a8633807dce8f61364ed0e3a701376aed69943fd2e6c7022846428d71ff77b7e50a1288e529\
1fc719d9b191a27099cc44c4c8ee0caa803f21b594cc91f74fcb93d57bdc46749189ef10247578713ea8de2ae9bfd645\
2c2bdae1fb73b765bed3660ed683823f9fdb799634a1a5611159c95935d3a5b1290be1f75f0fad1b49a9e67fc4f625e2\
d6075dfba30fce21701823fb55b618754ea67f52bada01f7d8391427a590fb48639aec0c417f3fc932ba09de9476a000\
a82acfc7c710e123b7d0859394fa95558d4de50332940a8bae0cf9f590d375415d42dfa32f417dbe680945043fd59a10\
30dfee31b13865613524ec58db093cac37925c516372c8670623736f1c0627d37fc34803ce48d8b1ad91fbd027913dc5\
9c00dd8a267a01e9460d3eb51bd45ccbcdf9c9c6f9f213e72a677c150c22d10c8641225979504f625dfedae2debf2c1a\
8d9e5e508dd1e82b5a036483d6a43170b06a67c074bde800dea1df64963ae981e0333b16f4ca63b7451e0e2833c8eb9b\
c828e8dd7a6eb7aa58bfd8af160e5a456107bbcc4b94e73b21f9bc2166a2d7f7acd81338cce2310597e5b8a79e17b2ed\
f7876ee7b0be032e995f5217553f700be324e7694433dbfcdd217270589a8b501e650ab7f5d0f2a5d4abc2a41564bdd2\
cd145b125a51516e5a87a28ef9304705be9760d41f417c3b3576619fbf49233ded63fd4ed6319a786c3618889ec87601\
49b3b23be33d177f52a395c9572000f70cf6376e2353e41d9512cb3ab95e078587e0d46f22fbf955a10fab4d1f45251c\
5bad9d81ba47d8484e089f16d52626ae366697cce2b7da9642f26b4aea4137cf5d62cc023e49a6ecd21b4fbcab06359d\
dc2dd1c3ed75e0e5037799bacdc30ae74b01eb24e659f12b078b1a9522078ee76843493fab8a8722f698b7a801216ceb\
07e60a12427459d51d80faa0b23e1f5f4640d0a89bfffb035d0de9a3e75d715b32d06c2711d36c462bf127f2054d088f\
1d867c6a8a8742544513d6d9ef5b525327037f8579c5da33299572d1ceacb4523d688ef4547f52c07638d12a03737e6e\
8634194bbb5afd5cdaa6de07e21432db59af7eeb77e520ecfc51ee544eb3f5590a9ac24d4d61da01dfb7db1b8fff311b\
6350fc21a0081bf16ffd1238dab8bb81c18b42719c73467367fa1654f6af0c85d719fc0205366af40331bb4a5d186c01\
0600008098b684d8314d5753000000000100000001000000000000000000000000000000000000000000000000000000\
0000000000ffffffff065104f09fa693ffffffff040065cd1d0000000017a91422333f916796a8cdb2656fa3fb0baa08\
13cb60f08740787d010000000017a91429e4c8c1df879c4452033b5e9a0f3185b8f1d6428780f0fa020000000017a914\
bf6506a42d05b142a49bb76de3476c0327ee77518780f0fa020000000017a914c7293839506014dc56917fc36d8174f1\
ee017f0c8700000000";
    const BLOCK_1_HASH: &str = "04513a2ca5e6c170ea402b309178ac024c040274e90fc176c2f5096156204bd7";
    const BLOCK_1_COINBASE_TXID: &str = "c2b01ae22bb0057503c67353b149f60b26ba72a0c4eb94f19632677772ef80b5";

    const BLOCK_94_HEX: &str = "04000000093ca387caa8afa39256935fae5ca0409b1e1dbea89887c9edb76d43d2bc8e01a3e2fa1f736496718a917bad\
3bdad71eb3ea594e5d551fcc76aecb0271c0696095215019efa864d4a0312a06c53505077703e75ba4c75da16f1af60e\
23952c5d00cab76a7f980220000000000000000000000000000000000000000000000000000000000000001cfd400500\
1b9143cfe78483a6e392651925b74db156ddeca320e45df4c8ab457b5b1c9454414ca3854d69b42aa708a523ae31588c\
adf12d849059a559f158863f84c82a36eee104b5054bacbb474e2ade52564e881f668804866051599db795220b030081\
9b325512dd28f7b4289c2eb2c168a3bfdb9f26b00b7d45ee9ef7372e7718ddff67d10b7022d21c146e95c2d33f9d793e\
3d6e29a7cebeacd33ae49cebb9f6bbec3cd6e75d9c413f02509e7d8c0c068d4683c3c2caa47d39b8ff13a1f92819eabf\
b952e880d3cdd3969bcad4c5f3551c87951108a53e4fd2280d6975b14334610eda4da8dae6b53ad705e71d7b43a1fbb3\
85862a5daec5e41c35963105fc04702d9b3e21e1a440db39afc164584ee3883b0716b11fc594aa69a0700348bdf15cc6\
7a73bcc02c0d59aad7e316b9eb0071e5b8b56d605dd2373e6cf25281b40148b76837f32a58fe65dcf3d2aacdf64bb300\
b67c9c70327c259ae0176a8afdd91680303a5ae4138f21fd8729ce9991bb81dac8c35349aebb96f1b100f44bf37e87d8\
3b288bf228c5372bf4b7fad45e145a870b9bb05a64639414caab92f8fec70780be5f5303eaecf1e481411a547c2079c4\
f822ccc42846382e07a25b2fb52ba757b5d250e425b7cd888976f3871d212661d95dd33495babde3f7a649d61d3139f4\
c523265f033e2ff67747d52482ec425ea4b68a2eff421d014f9dfe24429a4179c251a8a7dbc8be45a7d7f77803286db6\
bb5b209dade150877fc4fc702bed2d2d160ad9f370ef56ff4d82e6b10c7c7a0cb52a808d25a335a32d97c41e0ea31aed\
a4318de8bafe2923d70e9103eeb26e9a1fbb41b182b39c5cc76959209d6c673123a28658aa32dda1d21e1507394e0cad\
e14270f47b116c4efb96592a89e3a5d1fe26d90b9988a59ee5f71c7d0786ab95a28fe5fbe2b8ba45282eaf7f1d327600\
9bf9054d938608dff5463a8ef4a17aa487da9d4e05b09536af53dd55d858931acaa512c6d185bb8bc2133771cd0c48cc\
afb46a346bdbcdd23af816fa4a341a07aa8ff807c024d77d863969da6df395895f04361aaec3c2e49a7ac5b0dcf90668\
5d021b2114dfd6af2b9586c6708daee2913694fa9eb5bcadac9e5179f36398151c605b046bc612d6d5fc7aa217a3c65e\
2dfe6bec74efd7eef3a1b38c66c433e9aebe0f4bda6923059b709b4b4dcd21fa3da0cc518ba439f70a981e952e8d2ac4\
355d5fc7921fa4bdb0bde667d76cffbab30cc4c8fc8f0db33f4cfdd4bb525c0e2556dc11660520da4a6e46cdca149ea5\
841200e10359a31638cba60e4902efa147a011c25b22d6c399016e61725d61312f840498fcd225fd2cde73bbac4a6779\
5b59b14c862701a9f25bdb9ff3192343c557cf57e5bf46da75923816a6c73da0b739461c99cb9cfe7a5ac84a9f386001\
74ff75da160d9923ce758257437409b9e976ad0e0b3d1a81bb150cbbfb9d31c7b35d6bdea6adb94fa72013862fcd0806\
a173b7368d2f707e1db5a3dd6b9c2677b349dfd0d2cbffbd150816ec37962ca212b57c07c5dc4cd6c630cd9f64b2c9af\
cb4a517a163689022443ca18056171c3634c22d34aa196c1a5537da01c1c724a7c434819359d0fd46338c99176dadeba\
2f043ae123fb79dcb1779987b6308d60d87345415c70e00445bbbca7d20323ef83c20e9af0b41da52e4e7fdd333aa58e\
c04e5331dde895221db3b9fe2ae97f763508f83d108db22009e4ce76b0bcb7bb9215b65e173c0d2d01a82fb04319834d\
b27decc41e75659bb3a0af0d11748f7a45fd4d245ad6134cc7bdd25f215750841ea05290ed1392e2f55b364c67575c25\
ad74ce745e1959028f38fa73a9df93a79c8d701bbe2aa5985c641f4b7b41e80df45b34c5f204019b8688ca995cbdd001\
0600008098b684d8314d5753000000005e00000001000000000000000000000000000000000000000000000000000000\
0000000000ffffffff07015e04f09fa693ffffffff040065cd1d0000000017a91422333f916796a8cdb2656fa3fb0baa\
0813cb60f08740787d010000000017a91429e4c8c1df879c4452033b5e9a0f3185b8f1d6428780f0fa020000000017a9\
14bf6506a42d05b142a49bb76de3476c0327ee77518780f0fa020000000017a914c7293839506014dc56917fc36d8174\
f1ee017f0c8700000000";
    const BLOCK_94_HASH: &str = "01541db661fa78fa3877ff1b0dda0ac51b9feff9f58fbd1cba2f62326d00879e";
    const BLOCK_94_COINBASE_TXID: &str = "6069c07102cbae76cc1f555d4e59eab31ed7da3bad7b918a719664731ffae2a3";

    /// Deserializes a live block, checks the domain its coinbase names, and
    /// proves the block re-serializes byte for byte and hashes to what the
    /// node reports — header and coinbase txid both, so the ZIP-244 digest path
    /// through the vendored `zcash_primitives` is exercised for `SWM1` too.
    fn check(block_hex: &str, block_hash: &str, coinbase_txid: &str) -> Result<()> {
        let bytes = hex::decode(block_hex)?;
        let block = Block::zcash_deserialize(&bytes[..])?;

        assert_eq!(block.hash().to_string(), block_hash);
        assert_eq!(block.transactions.len(), 1, "coinbase only");
        let coinbase = &block.transactions[0];
        assert!(coinbase.is_coinbase());
        assert_eq!(coinbase.version(), 6);
        assert_eq!(coinbase.network_upgrade(), Some(NetworkUpgrade::SwarmMain));
        assert_eq!(coinbase.hash().to_string(), coinbase_txid);

        assert_eq!(block.zcash_serialize_to_vec()?, bytes, "byte-exact round trip");
        Ok(())
    }

    #[test]
    fn block_1_deserializes_round_trips_and_hashes() -> Result<()> {
        let _init_guard = zebra_test::init();
        check(BLOCK_1_HEX, BLOCK_1_HASH, BLOCK_1_COINBASE_TXID)
    }

    #[test]
    fn block_94_deserializes_round_trips_and_hashes() -> Result<()> {
        let _init_guard = zebra_test::init();
        check(BLOCK_94_HEX, BLOCK_94_HASH, BLOCK_94_COINBASE_TXID)
    }
}
