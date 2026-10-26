use crate::bitcoin::Transaction;
use crate::electrum::ElectrumClient;

use bdk_wallet::bitcoin::{
    absolute, transaction, Amount as BdkAmount, ScriptBuf as BdkScriptBuf,
    Transaction as BdkTransaction, TxOut as BdkTxOut,
};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

/// Starts a stand-in Electrum server that answers the `server.version` request the client sends
/// when it connects, then reads every other request without answering. Returns its URL.
fn start_silent_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let id = request_id(&line);
        writeln!(
            stream,
            r#"{{"jsonrpc":"2.0","id":{id},"result":["silent","1.4"]}}"#
        )
        .unwrap();
        line.clear();
        while reader.read_line(&mut line).is_ok_and(|n| n > 0) {
            line.clear();
        }
    });
    format!("tcp://{address}")
}

/// Reads the `id` of a JSON-RPC request.
fn request_id(request: &str) -> u64 {
    let start = request.find("\"id\":").expect("request has an id") + "\"id\":".len();
    request[start..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

fn transaction(sats: u64) -> Arc<Transaction> {
    Arc::new(Transaction::from(BdkTransaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: vec![],
        output: vec![BdkTxOut {
            value: BdkAmount::from_sat(sats),
            script_pubkey: BdkScriptBuf::new(),
        }],
    }))
}

#[test]
fn test_populate_tx_cache() {
    let client = ElectrumClient::new(start_silent_server(), None, Some(1), Some(0), true).unwrap();
    let cached = transaction(76_000);
    let not_cached = transaction(50_000);

    client.populate_tx_cache(vec![cached.clone()]);

    // The server never answers, so the first transaction can only come from the cache.
    let fetched = client.fetch_tx(cached.compute_txid()).unwrap();
    assert_eq!(fetched, cached);
    assert!(client.fetch_tx(not_cached.compute_txid()).is_err());
}
