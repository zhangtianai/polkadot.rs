// use jsonrpc_core_client::transports::ws::connect;
use ws::{connect, Result, Handler, Sender, Message, Handshake, CloseCode};
use serde_json::json;
use node_primitives::Hash;
use ws::{ErrorKind, Error};
use crossbeam;
use crossbeam::channel::{unbounded, Sender as ThreadOut};

pub mod utils;
use utils::*;

const WS_URL_LOCAL: &str = "ws://127.0.0.1:9944";

pub enum Url {
    Local,
    Custom(&'static str),
}

pub struct Api {
    url: String,
    genesis_hash: Hash,
}

impl Api {
    pub fn init(url: Url) -> Result<Self> {
        let json_req = json!({
            "method": "chain_getBlockHash",
            "params": [0],
            "jsonrpc": "2.0",
            "id": "1",
        });

        match url {
            Url::Local => {
                let genesis_hash_str = get_response(WS_URL_LOCAL, json_req.to_string())?;

                Ok(Api {
                    url: WS_URL_LOCAL.to_owned(),
                    genesis_hash: hexstr_to_hash(genesis_hash_str)
                })
            },
            Url::Custom(url) => {
                let genesis_hash_str = get_response(url, json_req.to_string())?;

                Ok(Api {
                    url: url.to_owned(),
                    genesis_hash: hexstr_to_hash(genesis_hash_str)
                })
            }
        }
    }

    pub fn get_storage(&self, module: &str, storage_key: &str, params: Option<Vec<u8>>) -> Result<String> {
        let key_hash = storage_key_hash(module, storage_key, params);
        let req = json!({
            "method": "state_getStorage",
            "params": [key_hash],
            "jsonrpc": "2.0",
            "id": "1",
        });

        get_response(&self.url[..], req.to_string())
    }
}

pub fn get_response(url: &str, req: String) -> Result<String> {
    let (tx, rx) = unbounded();

    crossbeam::scope(|scope| {
        scope.spawn(move |_| {
            connect(url.to_owned(), |output| {
                Getter {
                    output,
                    request: req.clone(),
                    result: tx.clone(),
                }
            }).expect("must connect")
        });
    }).expect("must run");

    Ok(rx.recv().expect("must not be empty"))
}

struct Getter {
    /// A representation of the output of the WebSocket connection.
    output: Sender,
    /// The json request data which is formatted string type.
    request: String,
    /// The sending side of a channel.
    result: ThreadOut<String>,
}

impl Handler for Getter {
    // Start handshake from clients
    fn on_open(&mut self, _: Handshake) -> Result<()> {
        self.output.send(self.request.clone())
            .map_err(|_| Error::new(ErrorKind::Internal, "must connect"))?;
        Ok(())
    }

    fn on_message(&mut self, msg: Message) -> Result<()> {
        let txt = msg.as_text()?;
        let value: serde_json::Value = serde_json::from_str(txt)
            .map_err(|_| Error::new(ErrorKind::Internal, "Request deserialization is infallible; qed"))?;

        let hex_str = match value["result"].as_str() {
            Some(res) => res.to_string(),
            None => "0x00".to_string(),
        };

        self.result.send(hex_str)
            .map_err(|_| Error::new(ErrorKind::Internal, "must run"))?;
        self.output.close(CloseCode::Normal)?;
        Ok(())
    }
}

struct Subscriber {

}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_get_storage() {
        let api = Api::init(Url::Local).unwrap();
        let res_str = api.get_storage("Balances", "transactionBaseFee", None).unwrap();
        let res = hexstr_to_u256(res_str);
        println!("TransactionBaseFee is {}", res);
    }
}
