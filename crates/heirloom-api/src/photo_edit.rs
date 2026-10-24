//! Photo edits (turn, straighten, trim) as new versions of the file; the original is kept.

use crate::{ApiError, ApiResult, Session};
use serde_json::Value;

pub fn call(_s: &mut Session, method: &str, _args: &Value) -> ApiResult {
    Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}")))
}
