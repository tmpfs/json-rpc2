//! Non-blocking implementation, requires the `async` feature.

use crate::{Request, Response, Error, Result};
use async_trait::async_trait;

#[async_trait]
/// Trait for async services that maybe handle a request.
pub trait Service {
    /// See [Service](crate::Service) for more information.
    async fn handle(&self, req: &mut Request) -> Result<Option<Response>>;
}

/// Call async services in order and return the first response message.
///
/// See [handle](crate::handle) for more information.
pub async fn handle<'a>(
    services: &'a Vec<&'a Box<dyn Service>>,
    request: &mut Request,
) -> Result<Response> {
    for service in services {
        if let Some(result) = service.handle(request).await? {
            return Ok(result);
        }
    }

    let err = Error::MethodNotFound {
        name: request.method().to_string(),
        id: request.id.clone(),
    };

    Ok((request, err).into())
}

/// Infallible async service handler, errors are automatically converted to responses.
pub async fn serve<'a>(
    services: &'a Vec<&'a Box<dyn Service>>,
    request: &mut Request,
) -> Response {
     match handle(services, request).await {
        Ok(response) => response,
        Err(e) => e.into(),
    }
}
