use serde::Serialize;

use crate::error::Error;

#[derive(Debug, Default, Serialize)]
pub struct NoData {}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Response<T: Serialize> {
    pub success: bool,
    pub elapsed_ms: u64,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

impl<T: Serialize> Response<T> {
    pub fn ok(data: T, elapsed_ms: u64) -> Self {
        Self {
            success: true,
            elapsed_ms,
            data: Some(data),
            error: None,
        }
    }
}

impl Response<NoData> {
    pub fn ok_empty(elapsed_ms: u64) -> Self {
        Self::ok(NoData::default(), elapsed_ms)
    }
}

impl Response<()> {
    pub fn from_error(err: &Error, elapsed_ms: u64) -> Self {
        Self {
            success: false,
            elapsed_ms,
            data: None,
            error: Some(ErrorPayload {
                code: err.code(),
                message: err.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize)]
    struct Stub {
        width: u32,
        height: u32,
    }

    #[test]
    fn response_ok_serializes_with_flattened_data() {
        let r = Response::ok(
            Stub {
                width: 800,
                height: 600,
            },
            12,
        );
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["elapsed_ms"], 12);
        assert_eq!(v["width"], 800);
        assert_eq!(v["height"], 600);
        assert!(v.get("error").is_none());
        assert!(v.get("data").is_none());
    }

    #[test]
    fn response_ok_empty_has_no_extra_fields() {
        let r = Response::<NoData>::ok_empty(5);
        let v = serde_json::to_value(&r).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(v["success"], true);
        assert_eq!(v["elapsed_ms"], 5);
    }

    #[test]
    fn response_from_error_serializes() {
        let r = Response::<()>::from_error(&Error::NotFound("missing".into()), 1);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["success"], false);
        assert_eq!(v["elapsed_ms"], 1);
        assert_eq!(v["error"]["code"], "NOT_FOUND");
        assert_eq!(v["error"]["message"], "not found: missing");
        assert!(v.get("data").is_none());
    }
}
