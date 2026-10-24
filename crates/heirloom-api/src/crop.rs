//! The crop of a person's profile photo (`CROP` under the person's link to the photo).

use crate::{ApiError, ApiResult, Session};
use serde_json::Value;

pub fn call(_s: &mut Session, method: &str, _args: &Value) -> ApiResult {
    Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}")))
}
