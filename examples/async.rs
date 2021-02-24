use async_trait::async_trait;
use json_rpc2::{futures::*, Context, Request, Response, Result};
use serde_json::Value;

struct ServiceHandler;

#[async_trait]
impl<T: Send + Sync> Service<T> for ServiceHandler {
    async fn handle(
        &self,
        req: &mut Request,
        _ctx: &Context<T>,
    ) -> Result<Option<Response>> {
        let mut response = None;
        if req.matches("hello") {
            let params: String = req.deserialize()?;
            let message = format!("Hello, {}!", params);
            response = Some((req, Value::String(message)).into());
        }
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service: Box<dyn Service<()>> = Box::new(ServiceHandler {});
    let mut request =
        Request::new("hello", Some(Value::String("world".to_string())));
    let server = Server::new(vec![&service]);
    let response = server.serve(&mut request, &Default::default()).await;
    println!("{:?}", response.result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.into()
    );
    Ok(())
}
