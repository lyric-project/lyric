use super::{Ctx, Handler};
use crate::capability::serialization::msgpack;
use async_trait::async_trait;
use serde_json::Value;

/// `wasi:logging/logging` implementation

#[async_trait]
impl<H: Handler> msgpack::Host for Ctx<H> {
    async fn serialize(&mut self, data: Vec<u8>) -> anyhow::Result<Result<Vec<u8>, String>> {
        let value: Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(e) => return Ok(Err(format!("Invalid JSON: {}", e))),
        };

        match rmp_serde::to_vec(&value) {
            Ok(buf) => Ok(Ok(buf)),
            Err(e) => Ok(Err(format!("Serialization error: {}", e))),
        }
    }

    async fn deserialize(&mut self, encoded: Vec<u8>) -> anyhow::Result<Result<Vec<u8>, String>> {
        match rmp_serde::from_slice::<Value>(&encoded) {
            Ok(value) => match serde_json::to_vec(&value) {
                Ok(json) => Ok(Ok(json)),
                Err(e) => Ok(Err(format!("JSON encoding error: {}", e))),
            },
            Err(e) => Ok(Err(format!("Deserialization error: {}", e))),
        }
    }
}
