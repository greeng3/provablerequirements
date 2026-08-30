//! Error type for tool + resource + prompt handlers.
//!
//! Surfaces up to the dispatcher where it's mapped onto JSON-RPC
//! errors. Kept narrow — most failures are either "bad args"
//! (InvalidParams, 400-ish) or "backend said no" (Upstream,
//! 500-ish); the dispatcher picks the right JSON-RPC code.

use thiserror::Error;

use crate::protocol::{JsonRpcErrorBody, error_codes};

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("{0}")]
    InvalidParams(String),
    #[error("upstream provreq server error: {0}")]
    Upstream(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl HandlerError {
    pub fn to_json_rpc(&self) -> JsonRpcErrorBody {
        match self {
            Self::InvalidParams(m) => JsonRpcErrorBody::new(error_codes::INVALID_PARAMS, m.clone()),
            Self::Upstream(m) => JsonRpcErrorBody::new(error_codes::INTERNAL_ERROR, m.clone()),
            Self::Internal(m) => JsonRpcErrorBody::new(error_codes::INTERNAL_ERROR, m.clone()),
        }
    }
}
