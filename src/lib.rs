#![deny(missing_docs)]
//! Simple and pragmatic facade for JSON-RPC2 services that is transport agnostic.
//!
//! ```
//! use json_rpc2::*;
//! use serde_json::Value;
//!
//! struct ServiceHandler;
//! impl Service for ServiceHandler {
//!    fn handle(&self, req: &mut Request) -> Result<Option<Response>> {
//!        let mut response = None;
//!        if req.matches("hello") {
//!            let params: String = req.into_params()?;
//!            let message = format!("Hello, {}!", params);
//!            response = Some((req, Value::String(message)).into());
//!        }
//!        Ok(response)
//!    }
//! }
//! 
//! fn main() -> Result<()> {
//!    let service: Box<dyn Service> = Box::new(ServiceHandler {});
//!    let mut request = Request::new(
//!        "hello", Some(Value::String("world".to_string())));
//!    let services = vec![&service];
//!    let response = match Broker::handle(&services, &mut request) {
//!        Ok(response) => response,
//!        Err(e) => e.into(),
//!    };
//!    println!("{:?}", response.result());
//!    assert_eq!(
//!        Some(Value::String("Hello, world!".to_string())),
//!        response.into_result());
//!    Ok(())
//! }
//! ```

use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Number, Value};

const VERSION: &str = "2.0";

const PARSE_ERROR: isize = -32700;
const INVALID_REQUEST: isize = -32600;
const METHOD_NOT_FOUND: isize = -32601;
const INVALID_PARAMS: isize = -32602;
const INTERNAL_ERROR: isize = -32603;

/// Result type for service handler functions and internal library errors.
pub type Result<T> = std::result::Result<T, Error>;

/// Enumeration of errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error generated when a JSON payload cannot be parsed.
    #[error("Parsing failed, invalid JSON data")]
    Parse {
        /// The underlying JSON error message.
        data: String
    },
    /// Error generated when the contents of a JSON payload do not 
    /// match the request type semantics.
    #[error("Invalid JSON-RPC request")]
    InvalidRequest {
        /// The underlying JSON error message.
        data: String
    },

    /// Error generated when the request method name did not 
    /// match any services.
    #[error("Service method not found: {name}")]
    MethodNotFound {
        /// The id of the request message.
        id: Value,
        /// The name of the request method.
        name: String
    },

    /// Error generated when request parameters cannot be converted 
    /// to the expected type.
    #[error("Message parameters are invalid")]
    InvalidParams {
        /// The id of the request message.
        id: Value,
        /// The underlying JSON error message.
        data: String
    },

    /// Generic JSON error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Generic error type.
    #[error(transparent)]
    Boxed(#[from] Box<dyn std::error::Error + Send>),
}

impl Error {
    /// Helper function to `Box` an error implementation.
    ///
    /// Useful in service handlers that need to use the `?` operator
    /// to propagate foreign errors via the service broker.
    pub fn boxed(e: impl std::error::Error + Send + 'static) -> Self {
        let err: Box<dyn std::error::Error + Send> = Box::new(e);
        Error::from(err)
    }
}

impl<'a> Into<Response> for (&'a mut Request, Error) {
    fn into(self) -> Response {
        let (code, data): (isize, Option<String>) = match &self.1 {
            Error::MethodNotFound { .. } => (METHOD_NOT_FOUND, None),
            Error::InvalidParams { data, .. } => {
                (INVALID_PARAMS, Some(data.to_string()))
            }
            Error::Parse { data } => (PARSE_ERROR, Some(data.to_string())),
            Error::InvalidRequest { data } => {
                (INVALID_REQUEST, Some(data.to_string()))
            }
            Error::Json(e) => (PARSE_ERROR, Some(e.to_string())),
            _ => (INTERNAL_ERROR, None),
        };
        Response {
            jsonrpc: VERSION.to_string(),
            id: self.0.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: self.1.to_string(),
                data,
            }),
        }
    }
}

impl Into<Response> for Error {
    fn into(self) -> Response {
        Response {
            jsonrpc: VERSION.to_string(),
            id: Value::Null,
            result: None,
            error: Some(JsonRpcError {
                code: INTERNAL_ERROR,
                message: self.to_string(),
                data: None,
            }),
        }
    }
}

/// Error information for response messages.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct JsonRpcError {
    code: isize,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

/// Trait for service handlers that maybe handle a request.
pub trait Service {
    /// Service implementations are invoked with a request 
    /// and should reply with a response if the method name 
    /// is one handled by the service.
    ///
    /// If the method name for the request is not handled by the service 
    /// if should return `None` so that the broker tries subsequent services.
    fn handle(&self, req: &mut Request) -> Result<Option<Response>>;
}

/// Broker calls multiple services and always yields a response.
pub struct Broker;
impl Broker {
    /// Call each of the services in order and return the 
    /// first response message.
    ///
    /// If no services match the incoming request this will 
    /// return a method not found response.
    pub fn handle<'a>(
        services: &'a Vec<&'a Box<dyn Service>>,
        req: &mut Request,
    ) -> Result<Response> {
        for service in services {
            if let Some(result) = service.handle(req)? {
                return Ok(result);
            }
        }

        let err = Error::MethodNotFound {
            name: req.method().to_string(),
            id: req.id.clone(),
        };

        Ok((req, err).into())
    }
}

/// JSON-RPC request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

impl Request {
    /// The id for the request.
    pub fn id(&self) -> &Value {
        &self.id
    }

    /// The request service method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Determine if the given name matches the request method.
    pub fn matches(&self, name: &str) -> bool {
        name == &self.method
    }

    /// Deserialize the message parameters into type `T`.
    ///
    /// If this request message has no parameters or the `params`
    /// payload cannot be converted to `T` this will return `INVALID_PARAMS`.
    pub fn into_params<T: DeserializeOwned>(&mut self) -> Result<T> {
        if let Some(params) = self.params.take() {
            Ok(serde_json::from_value::<T>(params).map_err(|e| {
                Error::InvalidParams {
                    id: self.id.clone(),
                    data: e.to_string(),
                }
            })?)
        } else {
            Err(Error::InvalidParams {
                id: self.id.clone(),
                data: "No parameters given".to_string(),
            })
        }
    }
}

fn map_json_error(e: serde_json::Error) -> Error {
    if e.is_syntax() {
        Error::Parse {
            data: e.to_string(),
        }
    } else if e.is_data() {
        Error::InvalidRequest {
            data: e.to_string(),
        }
    } else {
        Error::from(e)
    }
}

impl Request {
    /// Create a new request.
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            method: method.to_string(),
            params,
            id: Value::Number(Number::from(
                rand::thread_rng().gen_range(0..std::u32::MAX) + 1,
            )),
        }
    }

    /// Parse a JSON payload from a string slice.
    pub fn from_str(payload: &str) -> Result<Self> {
        Ok(serde_json::from_str::<Request>(payload).map_err(map_json_error)?)
    }

    /// Parse a JSON payload from a `Value`.
    pub fn from_value(payload: Value) -> Result<Self> {
        Ok(serde_json::from_value::<Request>(payload).map_err(map_json_error)?)
    }
}

/// JSON-RPC response.
#[derive(Deserialize, Serialize, Debug)]
pub struct Response {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

impl Response {
    /// The id for the response.
    pub fn id(&self) -> &Value {
        &self.id
    }

    /// The result for the response.
    pub fn result(&self) -> &Option<Value> {
        &self.result 
    }

    /// Convert into the result for this response.
    pub fn into_result(self) -> Option<Value> {
        self.result
    }

    /// The error for the response.
    pub fn error(&self) -> &Option<JsonRpcError> {
        &self.error
    }

    /// Convert into the error for this response.
    pub fn into_error(self) -> Option<JsonRpcError> {
        self.error
    }
}

impl<'a> From<(&'a mut Request, Value)> for Response {
    fn from(req: (&'a mut Request, Value)) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            id: req.0.id.clone(),
            result: Some(req.1),
            error: None,
        }
    }
}

impl<'a> From<&'a mut Request> for Response {
    fn from(req: &'a mut Request) -> Self {
        Self {
            jsonrpc: VERSION.to_string(),
            result: None,
            error: None,
            id: req.id.clone(),
        }
    }
}

mod test {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    enum MockError {
        #[error("{0}")]
        Internal(String),
    }

    struct HelloServiceHandler;
    impl Service for HelloServiceHandler {
        fn handle(&self, req: &mut Request) -> Result<Option<Response>> {
            let mut response = None;
            if req.matches("hello") {
                let params: String = req.into_params()?;
                let message = format!("Hello, {}!", params);
                response = Some((req, Value::String(message)).into());
            }
            Ok(response)
        }
    }

    struct InternalErrorService;
    impl Service for InternalErrorService {
        fn handle(&self, _req: &mut Request) -> Result<Option<Response>> {
            // Must Box the error as it is foreign.
            Err(Error::boxed(MockError::Internal("Mock error".to_string())))
        }
    }

    #[test]
    fn jsonrpc_service_ok() -> Result<()> {
        let service: Box<dyn Service> = Box::new(HelloServiceHandler {});
        let mut request = Request::new(
            "hello", Some(Value::String("world".to_string())));
        let services = vec![&service];
        let response = match Broker::handle(&services, &mut request) {
            Ok(response) => response,
            Err(e) => e.into(),
        };
        assert_eq!(
            Some(Value::String("Hello, world!".to_string())),
            response.into_result());
        Ok(())
    }

    #[test]
    fn jsonrpc_service_method_not_found() -> Result<()> {
        let service: Box<dyn Service> = Box::new(HelloServiceHandler {});
        let mut request = Request::new("non-existent", None);
        let services = vec![&service];
        let response = match Broker::handle(&services, &mut request) {
            Ok(response) => response,
            Err(e) => e.into(),
        };
        eprintln!("{:?}", response.error());
        assert_eq!(
            Some(JsonRpcError {
                code: -32601,
                message: "Service method not found: non-existent".to_string(),
                data: None}),
            response.into_error());
        Ok(())
    }

    #[test]
    fn jsonrpc_internal_error() -> Result<()> {
        let service: Box<dyn Service> = Box::new(InternalErrorService{});
        let mut request = Request::new("foo", None);
        let services = vec![&service];
        let response = match Broker::handle(&services, &mut request) {
            Ok(response) => response,
            Err(e) => e.into(),
        };

        assert_eq!(
            Some(JsonRpcError {
                code: -32603,
                message: "Mock error".to_string(),
                data: None}),
            response.into_error());
        
        Ok(())
    }
}
