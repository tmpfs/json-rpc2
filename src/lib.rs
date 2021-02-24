#![deny(missing_docs)]
//! Simple, robust and pragmatic facade for JSON-RPC2 services that is transport agnostic.
//!
//! ```
//! use json_rpc2::*;
//! use serde_json::Value;
//!
//! struct ServiceHandler;
//! impl<T> Service<T> for ServiceHandler {
//!    fn handle(&self, request: &mut Request, _ctx: &Context<T>) -> Result<Option<Response>> {
//!        let mut response = None;
//!        if request.matches("hello") {
//!            let params: String = request.deserialize()?;
//!            let message = format!("Hello, {}!", params);
//!            response = Some((request, Value::String(message)).into());
//!        }
//!        Ok(response)
//!    }
//! }
//!
//! fn main() -> Result<()> {
//!    let service: Box<dyn Service<()>> = Box::new(ServiceHandler {});
//!    let mut request = Request::new(
//!        "hello", Some(Value::String("world".to_string())));
//!    let server = Server::new(vec![&service]);
//!    let response = server.serve(&mut request, &Default::default());
//!    assert_eq!(
//!        Some(Value::String("Hello, world!".to_string())),
//!        response.into());
//!    Ok(())
//! }
//! ```
//!
//! ## Parsing
//!
//! When converting from incoming payloads use the `from_*` functions
//! to convert JSON to a [Request](Request) so that errors are mapped correctly.
//!
//! ## Async
//!
//! For nonblocking support enable the `async` feature and use the `Service`
//! trait from the `futures` module. You will also need to depend upon the
//! [async-trait](https://docs.rs/async-trait/0.1.42/async_trait/) crate and
//! use the `#[async_trait]` attribute macro on your service implementation.
//!
//! See the `async` example for usage.
//!

#[cfg(any(test, feature = "async"))]
pub mod futures;

use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Number, Value};

const VERSION: &str = "2.0";
const INVALID_REQUEST: isize = -32600;
const METHOD_NOT_FOUND: isize = -32601;
const INVALID_PARAMS: isize = -32602;
const INTERNAL_ERROR: isize = -32603;
const PARSE_ERROR: isize = -32700;

/// Result type for service handler functions and internal library errors.
pub type Result<T> = std::result::Result<T, Error>;

/// Enumeration of errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error generated when a JSON payload cannot be parsed.
    #[error("Parsing failed, invalid JSON data")]
    Parse {
        /// The underlying JSON error message.
        data: String,
    },
    /// Error generated when the contents of a JSON payload do not
    /// match the request type semantics.
    #[error("Invalid JSON-RPC request")]
    InvalidRequest {
        /// The underlying JSON error message.
        data: String,
    },

    /// Error generated when the request method name did not
    /// match any services.
    #[error("Service method not found: {name}")]
    MethodNotFound {
        /// The id of the request message.
        id: Value,
        /// The name of the request method.
        name: String,
    },

    /// Error generated when request parameters cannot be converted
    /// to the expected type.
    #[error("Message parameters are invalid")]
    InvalidParams {
        /// The id of the request message.
        id: Value,
        /// The underlying JSON error message.
        data: String,
    },

    /// Generic error type converted to an internal error response.
    #[error(transparent)]
    Boxed(#[from] Box<dyn std::error::Error + Send>),
}

impl Error {
    /// Helper function to `Box` an error implementation.
    ///
    /// Service handlers can call `map_err(Error::boxed)?` to propagate
    /// foreign errors.
    pub fn boxed(e: impl std::error::Error + Send + 'static) -> Self {
        let err: Box<dyn std::error::Error + Send> = Box::new(e);
        Error::from(err)
    }
}

impl<'a> Into<(isize, Option<String>)> for &'a Error {
    fn into(self) -> (isize, Option<String>) {
        match self {
            Error::MethodNotFound { .. } => (METHOD_NOT_FOUND, None),
            Error::InvalidParams { data, .. } => {
                (INVALID_PARAMS, Some(data.to_string()))
            }
            Error::Parse { data } => (PARSE_ERROR, Some(data.to_string())),
            Error::InvalidRequest { data } => {
                (INVALID_REQUEST, Some(data.to_string()))
            }
            _ => (INTERNAL_ERROR, None),
        }
    }
}

impl<'a> Into<Response> for (&'a mut Request, Error) {
    fn into(self) -> Response {
        let (code, data): (isize, Option<String>) = (&self.1).into();
        Response {
            jsonrpc: VERSION.to_string(),
            id: self.0.id.clone(),
            result: None,
            error: Some(RpcError {
                code,
                message: self.1.to_string(),
                data,
            }),
        }
    }
}

impl Into<Response> for Error {
    fn into(self) -> Response {
        let (code, data): (isize, Option<String>) = (&self).into();
        Response {
            jsonrpc: VERSION.to_string(),
            id: Value::Null,
            result: None,
            error: Some(RpcError {
                code,
                message: self.to_string(),
                data,
            }),
        }
    }
}

/// Error information for response messages.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct RpcError {
    /// The error code.
    pub code: isize,
    /// The error message.
    pub message: String,
    /// Additional data for the error, typically an underlying
    /// cause for the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Trait for services that maybe handle a request.
pub trait Service<T> {
    /// Service implementations are invoked with a request
    /// and should reply with a response if the method name
    /// is one handled by the service.
    ///
    /// If the method name for the request is not handled by the service
    /// if should return `None`.
    fn handle(
        &self,
        request: &mut Request,
        ctx: &Context<T>,
    ) -> Result<Option<Response>>;
}

/// Context information passed to service handlers that wraps user data.
#[derive(Default)]
pub struct Context<T> {
    /// Inner context data.
    pub data: T,
}

/// Serve requests.
pub struct Server<'a, T> {
    /// Services that the server should invoke for every request.
    services: Vec<&'a Box<dyn Service<T>>>,
}

impl<'a, T> Server<'a, T> {

    /// Create a new server.
    pub fn new(services: Vec<&'a Box<dyn Service<T>>>) -> Self {
        Self { services } 
    }

    /// Call services in order and return the first response message.
    ///
    /// If no services match the incoming request this will
    /// return a `Error::MethodNotFound`.
    pub(crate) fn handle(
        &self,
        request: &mut Request,
        ctx: &Context<T>,
    ) -> Result<Response> {
        for service in self.services.iter() {
            if let Some(result) = service.handle(request, ctx)? {
                return Ok(result);
            }
        }

        let err = Error::MethodNotFound {
            name: request.method().to_string(),
            id: request.id.clone(),
        };

        Ok((request, err).into())
    }

    /// Infallible service handler, errors are automatically converted to responses.
    pub fn serve(
        &self,
        request: &mut Request,
        ctx: &Context<T>,
    ) -> Response {
        match self.handle(request, ctx) {
            Ok(response) => response,
            Err(e) => e.into(),
        }
    }
}

/// Parse a JSON payload from a string slice into a request.
pub fn from_str(payload: &str) -> Result<Request> {
    Ok(serde_json::from_str::<Request>(payload).map_err(map_json_error)?)
}

/// Parse a JSON payload from a [Value](serde_json::Value) into a request.
pub fn from_value(payload: Value) -> Result<Request> {
    Ok(serde_json::from_value::<Request>(payload).map_err(map_json_error)?)
}

/// Parse a JSON payload from a byte slice into a request.
pub fn from_slice<'a>(payload: &'a [u8]) -> Result<Request> {
    Ok(serde_json::from_slice::<Request>(payload).map_err(map_json_error)?)
}

/// Parse a JSON payload from an IO reader into a request.
pub fn from_reader<R: std::io::Read>(payload: R) -> Result<Request> {
    Ok(serde_json::from_reader::<R, Request>(payload)
        .map_err(map_json_error)?)
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

    /// The id for the request.
    pub fn id(&self) -> &Value {
        &self.id
    }

    /// The request service method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// The request parameters.
    pub fn params(&self) -> &Option<Value> {
        &self.params
    }

    /// Determine if the given name matches the request method.
    pub fn matches(&self, name: &str) -> bool {
        name == &self.method
    }

    /// Deserialize and consume the message parameters into type `T`.
    ///
    /// If this request message has no parameters or the `params`
    /// payload cannot be converted to `T` this will return
    /// `Error::InvalidParams`.
    pub fn deserialize<T: DeserializeOwned>(&mut self) -> Result<T> {
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
    if e.is_data() {
        Error::InvalidRequest {
            data: e.to_string(),
        }
    } else {
        Error::Parse {
            data: e.to_string(),
        }
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
    error: Option<RpcError>,
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

    /// The error for the response.
    pub fn error(&self) -> &Option<RpcError> {
        &self.error
    }
}

impl Into<Option<Value>> for Response {
    fn into(self) -> Option<Value> {
        self.result
    }
}

impl Into<Option<RpcError>> for Response {
    fn into(self) -> Option<RpcError> {
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
    impl<T> Service<T> for HelloServiceHandler {
        fn handle(
            &self,
            request: &mut Request,
            _context: &Context<T>,
        ) -> Result<Option<Response>> {
            let mut response = None;
            if request.matches("hello") {
                let params: String = request.deserialize()?;
                let message = format!("Hello, {}!", params);
                response = Some((request, Value::String(message)).into());
            }
            Ok(response)
        }
    }

    struct InternalErrorService;
    impl<T> Service<T> for InternalErrorService {
        fn handle(
            &self,
            _request: &mut Request,
            _context: &Context<T>,
        ) -> Result<Option<Response>> {
            // Must Box the error as it is foreign.
            Err(Error::boxed(MockError::Internal("Mock error".to_string())))
        }
    }

    #[test]
    fn jsonrpc_service_ok() -> Result<()> {
        let service: Box<dyn Service<()>> = Box::new(HelloServiceHandler {});
        let mut request =
            Request::new("hello", Some(Value::String("world".to_string())));
        let server = Server::new(vec![&service]);
        let response = server.serve(&mut request, &Default::default());
        assert_eq!(
            Some(Value::String("Hello, world!".to_string())),
            response.into()
        );
        Ok(())
    }

    #[test]
    fn jsonrpc_invalid_request_error() -> Result<()> {
        let bad_json = "{}";
        let response: Response = match from_str(bad_json) {
            Ok(mut request) => (&mut request).into(),
            Err(e) => e.into(),
        };
        assert_eq!(
            Some(RpcError {
                code: -32600,
                message: "Invalid JSON-RPC request".to_string(),
                data: Some(
                    "missing field `jsonrpc` at line 1 column 2".to_string()
                )
            }),
            response.into()
        );
        Ok(())
    }

    #[test]
    fn jsonrpc_service_method_not_found() -> Result<()> {
        let service: Box<dyn Service<()>> = Box::new(HelloServiceHandler {});
        let mut request = Request::new("non-existent", None);
        let server = Server::new(vec![&service]);
        let response = server.serve(&mut request, &Default::default());
        assert_eq!(
            Some(RpcError {
                code: -32601,
                message: "Service method not found: non-existent".to_string(),
                data: None
            }),
            response.into()
        );
        Ok(())
    }

    #[test]
    fn jsonrpc_invalid_params() -> Result<()> {
        let service: Box<dyn Service<()>> = Box::new(HelloServiceHandler {});
        let mut request = Request::new("hello", Some(Value::Bool(true)));
        let server = Server::new(vec![&service]);
        let response = server.serve(&mut request, &Default::default());
        assert_eq!(
            Some(RpcError {
                code: -32602,
                message: "Message parameters are invalid".to_string(),
                data: Some(
                    "invalid type: boolean `true`, expected a string"
                        .to_string()
                )
            }),
            response.into()
        );
        Ok(())
    }

    #[test]
    fn jsonrpc_internal_error() -> Result<()> {
        let service: Box<dyn Service<()>> = Box::new(InternalErrorService {});
        let mut request = Request::new("foo", None);
        let server = Server::new(vec![&service]);
        let response = server.serve(&mut request, &Default::default());
        assert_eq!(
            Some(RpcError {
                code: -32603,
                message: "Mock error".to_string(),
                data: None
            }),
            response.into()
        );
        Ok(())
    }

    #[test]
    fn jsonrpc_parse_error() -> Result<()> {
        let bad_json = r#"{"jsonrpc": "oops}"#;
        let response: Response = match from_str(bad_json) {
            Ok(mut request) => (&mut request).into(),
            Err(e) => e.into(),
        };
        assert_eq!(
            Some(RpcError {
                code: -32700,
                message: "Parsing failed, invalid JSON data".to_string(),
                data: Some(
                    "EOF while parsing a string at line 1 column 18"
                        .to_string()
                )
            }),
            response.into()
        );
        Ok(())
    }
}
